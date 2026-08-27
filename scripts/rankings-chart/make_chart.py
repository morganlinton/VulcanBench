"""VulcanBench suite v3 rankings, four-panel shareable PNG.

Panel 1: pass@1 ranking bars (gradient, lab logo chips).
Panel 2: avg wall-clock minutes per task, fastest first.
Panel 3: avg API cost per task run, lowest first.
Panel 4: effort-curve cards per swept model.
"""

import json
import math
from pathlib import Path

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
from matplotlib import font_manager
from matplotlib.colors import LinearSegmentedColormap, to_rgb
from matplotlib.offsetbox import AnnotationBbox, OffsetImage
from matplotlib.patches import FancyBboxPatch, PathPatch, Polygon, Rectangle
from matplotlib.path import Path as MplPath
from matplotlib.transforms import blended_transform_factory

HERE = Path(__file__).resolve().parent

for w in (400, 500, 600, 700):
    font_manager.fontManager.addfont(str(HERE / f"geist-{w}.ttf"))
for w in (400, 500, 600):
    font_manager.fontManager.addfont(str(HERE / f"chakra-{w}.ttf"))
matplotlib.rcParams["font.family"] = "Geist"
SANS = "Geist"
BRAND = "Chakra Petch SemiBold"  # vulcanbench.com wordmark face
BRAND_MED = "Chakra Petch Medium"

SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#8a897f"
GRID = "#e7e6e1"

# Brand colors: Anthropic clay, OpenAI green, DeepSeek blue (official icon
# colors); xAI's official mark is black. Moonshot's is also black, so it gets a
# dark slate instead, two identical black bar families would be unreadable.
LAB_COLOR = {
    "Anthropic": "#D97757",
    "OpenAI": "#10A37F",
    "xAI": "#0A0A0A",
    "Moonshot": "#44445E",
    "DeepSeek": "#5786FE",
    # Qwen's official violet (#6950EF) sits ΔE 12 from DeepSeek's blue, below
    # the readability floor, so this deepened violet stands in for it.
    "Alibaba": "#9333EA",
    # Z.ai: deep teal, dE > 15 from every neighbour; all marks stay labelled.
    "Z.ai": "#0e7a8a",
    "Meta": "#0866ff",
}

NAME = {
    "xai:grok-4.6": ("Grok 4.6", "xAI"),
    "openai:grok-4.5": ("Grok 4.5", "xAI"),
    "openai:gpt-5.6-luna": ("GPT-5.6 Luna", "OpenAI"),
    "openai:gpt-5.6-sol": ("GPT-5.6 Sol", "OpenAI"),
    "openai:gpt-5.6-terra": ("GPT-5.6 Terra", "OpenAI"),
    "anthropic:claude-fable-5": ("Claude Fable 5", "Anthropic"),
    "anthropic:claude-haiku-4-5": ("Claude Haiku 4.5", "Anthropic"),
    "anthropic:claude-opus-4-8": ("Claude Opus 4.8", "Anthropic"),
    "anthropic:claude-opus-5": ("Claude Opus 5", "Anthropic"),
    "deepseek:deepseek-v4-flash": ("DeepSeek V4-Flash", "DeepSeek"),
    "deepseek:deepseek-v4-pro": ("DeepSeek V4 Pro", "DeepSeek"),
    "kimi:kimi-k3": ("Kimi K3", "Moonshot"),
    "qwen:qwen3.8-max": ("Qwen3.8-Max", "Alibaba"),
    "qwen:qwen3.8-27b": ("Qwen3.8-27B", "Alibaba"),
    "zai:glm-5.3": ("GLM 5.3", "Z.ai"),
    "meta:muse-spark-1.2": ("Muse Spark 1.2", "Meta"),
}


def lighten(hex_color: str, f: float) -> tuple:
    r, g, b = to_rgb(hex_color)
    return (r + (1 - r) * f, g + (1 - g) * f, b + (1 - b) * f)


def darken(hex_color: str, f: float) -> tuple:
    r, g, b = to_rgb(hex_color)
    return (r * (1 - f), g * (1 - f), b * (1 - f))


def eff_display(model: str, eff: str) -> str:
    """Label an effort with the provider's own name for it."""
    if eff in ("", "\u2014", "-"):  # stored as a dash when no effort was set
        return "default"
    if eff == "extra-high":
        if model.startswith(("deepseek:", "zai:")):
            return "max"
        if model.startswith(("qwen:", "xai:", "meta:")):
            return "xhigh"
    return eff


with open(HERE / "v3_rankings.json") as f:
    rows = json.load(f)
# Deliberate exclusions, not silent drops: Opus 4.8 has 5/23 task coverage.
# (Muse Spark's Meta-direct ladder joined 2026-08-25 from Report 19; the old
# OpenRouter-routed partial rows never entered this file.)
EXCLUDED_MODELS = {
    "anthropic:claude-opus-4-8",
    "zcode:glm-5.3",  # subscription harness: model plus product, off the board
    "pi:meta:muse-spark-1.2",  # Pi harness: model plus agent, off the board
    "ollama:qwen3.8:27b",  # local-inference control runs
}
rows = [r for r in rows if r["model"] not in EXCLUDED_MODELS]
unknown = {r["model"] for r in rows} - set(NAME)
if unknown:
    raise SystemExit(f"models missing from NAME (add or exclude explicitly): {unknown}")


def model_efforts(model: str) -> list[str]:
    """The provider's own effort ladder, low to high."""
    if model.startswith("deepseek:"):
        return ["low", "high", "extra-high"]  # DeepSeek: low/high/max
    if model.startswith("qwen:"):
        return ["low", "medium", "extra-high"]  # Qwen: low/medium/xhigh
    if model.startswith("zai:"):
        return ["low", "high", "extra-high"]  # GLM 5.3: low/high/max
    if model.startswith("meta:"):
        return ["low", "high", "extra-high"]  # Muse Spark: low/high/xhigh
    if model.startswith("xai:"):
        # Grok 4.6+: low/medium/high/xhigh, four real levels; dropping xhigh
        # would hide the curve's shape (medium peak, high trough, xhigh partial
        # recovery).
        return ["low", "medium", "high", "extra-high"]
    return ["low", "medium", "high"]


def best_effort_row(model_rows: list[dict]) -> dict:
    """The model's best-scoring effort column; ties break to the cheaper run.

    Full-coverage (23-task) columns are preferred; a partial column can only
    win when the model has no full column. The bar panels show one column per
    model; the full per-effort data stays visible in the effort-curve cards.
    """
    full = [r for r in model_rows if r["n_tasks"] >= 23]
    pool = full or model_rows
    return max(pool, key=lambda r: (r["pass1"], -r["cost"] / max(r["n_runs"], 1)))


# `rows` keeps every (model, effort) column for the effort-curve cards;
# `bar_rows` is one column per model (its best-scoring effort) for the three bar panels.
rows.sort(key=lambda r: (-r["pass1"], r["cost"]))
_per_model: dict[str, list[dict]] = {}
for r in rows:
    _per_model.setdefault(r["model"], []).append(r)
bar_rows = [best_effort_row(v) for v in _per_model.values()]
bar_rows.sort(key=lambda r: (-r["pass1"], r["cost"]))

fig = plt.figure(figsize=(16, 30), facecolor=SURFACE)
gs = fig.add_gridspec(
    4,
    1,
    height_ratios=[1.1, 0.62, 0.62, 1.55],
    hspace=0.72,
    left=0.065,
    right=0.955,
    top=0.884,
    bottom=0.092,
)

LOGOS = {lab: plt.imread(str(HERE / f"logos/{lab}.png")) for lab in LAB_COLOR}
grad = np.linspace(0, 1, 256).reshape(-1, 1)


def top_rounded_bar(x: float, height: float, width: float, ymax: float) -> PathPatch:
    """A data-space bar with a square baseline and subtly rounded top corners."""
    x0, x1 = x - width / 2, x + width / 2
    rx = width * 0.28
    ry = min(ymax * 0.018, height * 0.45)
    k = 0.55228475  # cubic approximation of a quarter ellipse
    vertices = [
        (x0, 0),
        (x0, height - ry),
        (x0, height - ry * (1 - k)),
        (x0 + rx * (1 - k), height),
        (x0 + rx, height),
        (x1 - rx, height),
        (x1 - rx * (1 - k), height),
        (x1, height - ry * (1 - k)),
        (x1, height - ry),
        (x1, 0),
        (x0, 0),
    ]
    codes = [
        MplPath.MOVETO,
        MplPath.LINETO,
        MplPath.CURVE4,
        MplPath.CURVE4,
        MplPath.CURVE4,
        MplPath.LINETO,
        MplPath.CURVE4,
        MplPath.CURVE4,
        MplPath.CURVE4,
        MplPath.LINETO,
        MplPath.CLOSEPOLY,
    ]
    return PathPatch(
        MplPath(vertices, codes),
        facecolor="none",
        edgecolor="none",
        transform=None,
    )


def draw_bars(
    ax, bar_rows, values, val_fmt, ymax, ytick_step, ylabel, errs=None, value_fontsize=15
):
    """Gradient bars + logo chips + shared axis cosmetics (panels 1-3)."""
    xs = list(range(len(bar_rows)))
    W = 0.62
    for x, v, r in zip(xs, values, bar_rows, strict=True):
        _, lab = NAME[r["model"]]
        c = LAB_COLOR[lab]
        cmap = LinearSegmentedColormap.from_list(
            f"g{id(ax)}{x}", [darken(c, 0.18), to_rgb(c), lighten(c, 0.42)]
        )
        clip = top_rounded_bar(x, v, W, ymax)
        clip.set_transform(ax.transData)
        ax.add_patch(clip)
        img = ax.imshow(
            grad,
            extent=(x - W / 2, x + W / 2, 0, v),
            origin="lower",
            aspect="auto",
            cmap=cmap,
            zorder=3,
            interpolation="bicubic",
        )
        img.set_clip_path(clip)
        ax.add_patch(
            Rectangle(
                (x - W / 2, 0),
                W,
                min(ymax * 0.028, v),
                facecolor=darken(c, 0.18),
                edgecolor="none",
                zorder=3,
            )
        )
        top = v
        if errs is not None and errs[x]:
            se = errs[x]
            ax.plot(
                [x, x],
                [max(0, v - se), min(ymax * 0.985, v + se)],
                color=INK2,
                linewidth=1.7,
                alpha=0.55,
                zorder=5,
                solid_capstyle="round",
            )
            top = v + se
        ax.text(
            x,
            top + ymax * 0.013,
            val_fmt(v),
            ha="center",
            va="bottom",
            fontsize=value_fontsize,
            color=INK,
            fontweight="bold",
            family=SANS,
        )

    badge_t = blended_transform_factory(ax.transData, ax.transAxes)
    for x, r in zip(xs, bar_rows, strict=True):
        _, lab = NAME[r["model"]]
        ax.add_patch(
            FancyBboxPatch(
                (x - 0.23, -0.098),
                0.46,
                0.073,
                boxstyle="round,pad=0,rounding_size=0.12",
                mutation_aspect=0.155,
                transform=badge_t,
                facecolor=LAB_COLOR[lab],
                edgecolor="none",
                clip_on=False,
                zorder=6,
            )
        )
        img = LOGOS[lab]
        h, w = img.shape[:2]
        zoom = min(24.0 / w, 14.5 / h)
        ab = AnnotationBbox(
            OffsetImage(img, zoom=zoom, interpolation="lanczos"),
            (x, -0.061),
            xycoords=badge_t,
            frameon=False,
            box_alignment=(0.5, 0.5),
            annotation_clip=False,
        )
        ab.set_zorder(7)
        ax.add_artist(ab)

    ax.set_xticks(xs)
    ax.set_xlim(-0.7, len(bar_rows) - 0.3)
    ax.set_ylim(0, ymax)
    tick_top = math.floor(ymax * 0.96 / ytick_step) * ytick_step
    ax.set_yticks(np.arange(0, tick_top + ytick_step / 2, ytick_step))
    ax.tick_params(axis="y", labelsize=10, colors=MUTED, length=0)
    ax.tick_params(axis="x", length=0, pad=38)
    ax.grid(axis="y", color=GRID, linewidth=0.9, linestyle=(0, (1, 4)), zorder=0)
    for s in ax.spines.values():
        s.set_visible(False)
    ax.set_ylabel(ylabel, fontsize=11, color=INK2, family=SANS)


# ---------------- Header ----------------
vb_logo = plt.imread(str(HERE / "vb_logo_rounded.png"))
ab = AnnotationBbox(
    OffsetImage(vb_logo, zoom=0.0295, interpolation="lanczos"),
    (0.079, 0.975),
    xycoords=fig.transFigure,
    frameon=False,
    box_alignment=(0.5, 0.5),
)
fig.add_artist(ab)
fig.text(0.098, 0.9662, "VulcanBench", fontsize=29, color=INK, family=BRAND)
fig.text(0.30, 0.9662, "Eval Suite 3, Model Rankings", fontsize=29, color=MUTED, family=BRAND_MED)
fig.text(
    0.065,
    0.9424,
    "23 frontier-hard software-engineering tasks from real merged OSS PRs  ·  "
    "pass@1 at each model's best-scoring reasoning effort  ·  Docker-sandboxed agent runs  ·  "
    "2026-08-25",
    fontsize=11.5,
    color=INK2,
    family=SANS,
)

# ---------------- Panel 1: pass@1 rankings ----------------
ax1 = fig.add_subplot(gs[0])
ax1.set_facecolor(SURFACE)
labels1 = []
for r in bar_rows:
    disp, _ = NAME[r["model"]]
    partial = "*" if r["n_tasks"] < 23 else ""
    labels1.append(
        f"{disp} ({eff_display(r['model'], r['effort'])}){partial}"
        f"  ·  ${r['cost']:.2f}  ·  n={r['n_runs']}"
    )
draw_bars(
    ax1,
    bar_rows,
    [r["pass1"] * 100 for r in bar_rows],
    lambda v: f"{v:.0f}",
    108,
    20,
    "pass@1 (%)",
    errs=[(r.get("se") or 0) * 100 for r in bar_rows],
)
ax1.set_xticklabels(
    labels1, rotation=47, ha="right", rotation_mode="anchor", fontsize=10, color=INK2, family=SANS
)

seen = dict.fromkeys(NAME[r["model"]][1] for r in bar_rows)
handles = [plt.Rectangle((0, 0), 1, 1, color=LAB_COLOR[lab]) for lab in seen]
ax1.legend(
    handles,
    list(seen),
    loc="upper right",
    frameon=False,
    ncol=len(seen),
    bbox_to_anchor=(1.005, 1.14),
    handlelength=1.1,
    handleheight=1.1,
    columnspacing=1.4,
    prop={"family": SANS, "size": 11.5},
    labelcolor=INK2,
)
ax1.set_title(
    "Rankings by pass@1, best effort level per model",
    loc="left",
    fontsize=16,
    color=INK,
    pad=48,
    family=BRAND_MED,
)

# ---------------- Panel 2: minutes per task, fastest first ----------------
ax2 = fig.add_subplot(gs[1])
ax2.set_facecolor(SURFACE)
trows = sorted(bar_rows, key=lambda r: r["avg_duration_s"] or 0)
tvals = [(r["avg_duration_s"] or 0) / 60 for r in trows]
labels2 = []
for r in trows:
    disp, _ = NAME[r["model"]]
    partial = "*" if r["n_tasks"] < 23 else ""
    labels2.append(f"{disp} ({eff_display(r['model'], r['effort'])}){partial}")
tmax = max(tvals) * 1.22
draw_bars(
    ax2, trows, tvals, lambda v: f"{v:.1f}m" if v < 10 else f"{v:.0f}m", tmax, 5, "min / task"
)
ax2.set_xticklabels(
    labels2, rotation=47, ha="right", rotation_mode="anchor", fontsize=10, color=INK2, family=SANS
)
ax2.set_title(
    "Speed, avg wall-clock minutes per task, fastest first",
    loc="left",
    fontsize=16,
    color=INK,
    pad=16,
    family=BRAND_MED,
)


# ---------------- Panel 3: average API cost per task run ----------------
ax3 = fig.add_subplot(gs[2])
ax3.set_facecolor(SURFACE)
crows = sorted(bar_rows, key=lambda r: r["cost"] / r["n_runs"])
cvals = [r["cost"] / r["n_runs"] for r in crows]
labels3 = []
for r in crows:
    disp, _ = NAME[r["model"]]
    partial = "*" if r["n_tasks"] < 23 else ""
    labels3.append(f"{disp} ({eff_display(r['model'], r['effort'])}){partial}")
cmax = max(cvals) * 1.22
draw_bars(
    ax3,
    crows,
    cvals,
    lambda v: f"${v:.2f}",
    cmax,
    0.5,
    "$ / task run",
    value_fontsize=12,
)
ax3.set_xticklabels(
    labels3,
    rotation=47,
    ha="right",
    rotation_mode="anchor",
    fontsize=10,
    color=INK2,
    family=SANS,
)
ax3.set_title(
    "Cost, avg API spend per task run, lowest first",
    loc="left",
    fontsize=16,
    color=INK,
    pad=16,
    family=BRAND_MED,
)


# ---------------- Panel 4: effort-curve cards ----------------
by_model: dict[str, dict[str, dict]] = {}
for r in rows:
    if r["effort"] in model_efforts(r["model"]):
        by_model.setdefault(r["model"], {})[r["effort"]] = r
swept = [m for m, effs in by_model.items() if len(effs) >= 2]
swept.sort(key=lambda m: -max(e["pass1"] for e in by_model[m].values()))

card_cols = 4
card_rows = math.ceil(len(swept) / card_cols)
gs2 = gs[3].subgridspec(card_rows, card_cols, wspace=0.16, hspace=0.22)
CARD = "#f6f5f1"
CARD_EDGE = "#e8e6df"
# Cards share one y-scale so their shapes stay comparable; the floor follows
# the lowest point (with room for its whisker) instead of clipping it.
_card_lows = [(e["pass1"] - (e.get("se") or 0)) * 100 for m in swept for e in by_model[m].values()]
_card_highs = [(e["pass1"] + (e.get("se") or 0)) * 100 for m in swept for e in by_model[m].values()]
Y0 = min(73, 5 * math.floor((min(_card_lows) - 2) / 5))
# Keep the top point under ~85% of the card so the header row stays clear.
Y1 = max(97, Y0 + (max(_card_highs) - Y0) / 0.84)
_card_ticks = list(range(int(math.ceil(Y0 / 5) * 5), int(max(_card_highs)) + 1, 5))
first_card = None
for k, model in enumerate(swept):
    axc = fig.add_subplot(gs2[k // card_cols, k % card_cols])
    if first_card is None:
        first_card = axc
    axc.set_facecolor("none")
    disp, lab = NAME[model]
    c = LAB_COLOR[lab]
    effs = by_model[model]
    EFFORTS = model_efforts(model)

    axc.add_patch(
        FancyBboxPatch(
            (0.0, 0.0),
            1.0,
            1.0,
            boxstyle="round,pad=0.03,rounding_size=0.06",
            transform=axc.transAxes,
            facecolor=CARD,
            edgecolor=CARD_EDGE,
            linewidth=1.2,
            zorder=0,
            clip_on=False,
        )
    )

    pts = [(EFFORTS.index(e), effs[e]["pass1"] * 100) for e in EFFORTS if e in effs]
    px, py = zip(*pts, strict=True)

    poly = Polygon(
        [*pts, (px[-1], Y0), (px[0], Y0)],
        closed=True,
        transform=axc.transData,
        facecolor="none",
        edgecolor="none",
    )
    axc.add_patch(poly)
    ga = np.zeros((256, 1, 4))
    ga[..., :3] = to_rgb(c)
    ga[..., 3] = np.linspace(0.02, 0.42, 256).reshape(-1, 1)
    gi = axc.imshow(
        ga,
        extent=(px[0], px[-1], Y0, max(py)),
        origin="lower",
        aspect="auto",
        zorder=1,
        interpolation="bilinear",
    )
    gi.set_clip_path(poly)

    # ±1 stderr whiskers (per-task-mean stderr; single runs use binomial se).
    for e in EFFORTS:
        if e in effs and effs[e].get("se"):
            i, v, se = EFFORTS.index(e), effs[e]["pass1"] * 100, effs[e]["se"] * 100
            axc.plot(
                [i, i],
                [v - se, v + se],
                color=c,
                linewidth=1.6,
                alpha=0.45,
                zorder=2,
                solid_capstyle="round",
            )

    for lw, al in ((9, 0.10), (5.5, 0.18)):
        axc.plot(px, py, color=c, linewidth=lw, alpha=al, zorder=2, solid_capstyle="round")
    axc.plot(px, py, color=c, linewidth=2.8, zorder=3, solid_capstyle="round")
    axc.scatter(px, py, s=90, color=c, zorder=4, edgecolors="white", linewidths=2)

    for i, v in pts:
        axc.annotate(
            f"{v:.0f}",
            (i, v),
            textcoords="offset points",
            xytext=(0, 12),
            ha="center",
            fontsize=12.5,
            color=INK,
            fontweight="bold",
            family=SANS,
            zorder=5,
        )

    axc.add_patch(
        FancyBboxPatch(
            (0.045, 0.865),
            0.115,
            0.105,
            boxstyle="round,pad=0,rounding_size=0.03",
            transform=axc.transAxes,
            facecolor=c,
            edgecolor="none",
            zorder=5,
        )
    )
    img = LOGOS[lab]
    h, w = img.shape[:2]
    zoom = min(15.0 / w, 10.0 / h)
    ab = AnnotationBbox(
        OffsetImage(img, zoom=zoom, interpolation="lanczos"),
        (0.102, 0.917),
        xycoords=axc.transAxes,
        frameon=False,
        box_alignment=(0.5, 0.5),
    )
    ab.set_zorder(6)
    axc.add_artist(ab)
    partial = "*" if any(effs[e]["n_tasks"] < 23 for e in effs) else ""
    axc.text(
        0.20,
        0.917,
        f"{disp}{partial}",
        transform=axc.transAxes,
        fontsize=12.5,
        color=INK,
        family=SANS,
        fontweight="bold",
        va="center",
    )

    tick_names = {"low": "Low", "medium": "Med", "high": "High"}
    axc.set_xlim(-0.42, len(EFFORTS) - 1 + 0.42)
    axc.set_ylim(Y0, Y1)
    axc.set_xticks(range(len(EFFORTS)))
    axc.set_xticklabels(
        [tick_names.get(e, eff_display(model, e).capitalize()) for e in EFFORTS],
        fontsize=10.5,
        color=INK2,
        family=SANS,
    )
    axc.grid(axis="y", color="#dddbd3", linewidth=0.8, linestyle=(0, (1, 4)), zorder=0)
    axc.set_yticks(_card_ticks)
    if k % card_cols == 0:
        axc.tick_params(axis="y", labelsize=9.5, colors=MUTED, length=0)
        axc.set_ylabel("pass@1 (%)", fontsize=10.5, color=INK2, family=SANS)
    else:
        axc.set_yticklabels([])
        axc.tick_params(axis="y", length=0)
    axc.tick_params(axis="x", length=0, pad=8)
    for s in axc.spines.values():
        s.set_visible(False)

card_top = first_card.get_position().y1
fig.text(
    0.065,
    card_top + 0.012,
    "Effort curves, how pass@1 responds to reasoning effort",
    fontsize=16,
    color=INK,
    family=BRAND_MED,
)

# ---------------- Footnote ----------------
fig.text(
    0.065,
    0.062,
    "* partial coverage, Claude Fable 5 excludes tasks refused by safety filters "
    "(low 19/23, medium 21/23, high 20/23); Kimi K3 19/23; Claude Haiku 4.5 21/23. "
    "Grok 4.5 low has 21/23 fresh tasks; Claude Opus 4.8 omitted (5/23 tasks).\n"
    "Claude Opus 5 columns are from vulcanbench.com Report 10 (single runs, "
    "2026-07-26); 4 of its 5 high-effort failures were wall-clock timeouts. "
    "Haiku 4.5 (default) and Kimi K3 (extra-high) have no effort sweep.\n"
    "DeepSeek's effort scale is low/high/max per its API; an accidental duplicate "
    "high run (its API coerces 'medium' to high) is excluded. Qwen's scale is "
    "low/medium/xhigh (no 'high').\n"
    "Repeat-swept models aggregate all fresh runs as per-task means; exact run "
    "counts are shown on labels. GPT-5.6 Terra and Luna each have 3 runs/task "
    "at low/medium/high.\n"
    "Five Qwen xhigh runs on 3 tasks were lost to 600s API read timeouts and "
    "are excluded rather than scored 0. "
    "Grok 4.6 columns are a single pass (repeat 1, 2026-08-12) on xAI's API; its "
    "effort scale is low/medium/high/xhigh and its unset-effort default is high. "
    "Qwen3.8-27B (Report 17), GLM 5.3 (Report 18), and Muse Spark 1.2 (Report 19) are "
    "single passes per level with median times; GLM's scale is low/high/max, its unset default is max, and its "
    "ZCode-harness results (model plus product) are excluded from the board.\n"
    "Bar panels show one column per model at its best-scoring effort (ties to the cheaper run); every effort "
    "level is plotted in the effort-curve cards.\n"
    "Whiskers are ±1 stderr, single-pass columns (n=23 runs or fewer) carry wider "
    "uncertainty than repeat-swept ones (n=52-71). Cost = total spend at list API "
    "prices across a column's runs; avg cost/task run = column cost ÷ n. Time = "
    "sandbox wall-clock.\n"
    "github.com/morganlinton/VulcanBench",
    fontsize=9,
    color=MUTED,
    ha="left",
    family=SANS,
    linespacing=1.45,
    va="top",
)

fig.savefig(HERE / "vulcanbench_suite3_rankings.png", dpi=160, facecolor=SURFACE)
print("saved")

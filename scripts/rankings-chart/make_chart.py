"""VulcanBench suite v3 rankings — three-panel shareable PNG.

Panel 1: pass@1 ranking bars (gradient, lab logo chips).
Panel 2: avg wall-clock minutes per task, fastest first.
Panel 3: effort-curve cards per swept model.
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
from matplotlib.patches import FancyBboxPatch, Polygon, Rectangle
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
# dark slate instead — two identical black bar families would be unreadable.
LAB_COLOR = {
    "Anthropic": "#D97757",
    "OpenAI": "#10A37F",
    "xAI": "#0A0A0A",
    "Moonshot": "#44445E",
    "DeepSeek": "#5786FE",
    # Qwen's official violet (#6950EF) sits ΔE 12 from DeepSeek's blue — below
    # the readability floor — so this deepened violet stands in for it.
    "Alibaba": "#9333EA",
}

NAME = {
    "openai:grok-4.5": ("Grok 4.5", "xAI"),
    "openai:gpt-5.6-sol": ("GPT-5.6 Sol", "OpenAI"),
    "anthropic:claude-fable-5": ("Claude Fable 5", "Anthropic"),
    "anthropic:claude-haiku-4-5": ("Claude Haiku 4.5", "Anthropic"),
    "anthropic:claude-opus-4-8": ("Claude Opus 4.8", "Anthropic"),
    "anthropic:claude-opus-5": ("Claude Opus 5", "Anthropic"),
    "deepseek:deepseek-v4-flash": ("DeepSeek V4-Flash", "DeepSeek"),
    "kimi:kimi-k3": ("Kimi K3", "Moonshot"),
    "qwen:qwen3.8-max": ("Qwen3.8-Max", "Alibaba"),
}


def lighten(hex_color: str, f: float) -> tuple:
    r, g, b = to_rgb(hex_color)
    return (r + (1 - r) * f, g + (1 - g) * f, b + (1 - b) * f)


def darken(hex_color: str, f: float) -> tuple:
    r, g, b = to_rgb(hex_color)
    return (r * (1 - f), g * (1 - f), b * (1 - f))


def eff_display(model: str, eff: str) -> str:
    """Label an effort with the provider's own name for it."""
    if eff == "—":
        return "default"
    if eff == "extra-high":
        if model.startswith("deepseek:"):
            return "max"
        if model.startswith("qwen:"):
            return "xhigh"
    return eff


with open(HERE / "v3_rankings.json") as f:
    rows = json.load(f)
rows = [r for r in rows if r["model"] != "anthropic:claude-opus-4-8"]
rows.sort(key=lambda r: (-r["pass1"], r["cost"]))

fig = plt.figure(figsize=(16, 22.5), facecolor=SURFACE)
gs = fig.add_gridspec(
    3,
    1,
    height_ratios=[1.1, 0.72, 1.0],
    hspace=0.78,
    left=0.065,
    right=0.955,
    top=0.884,
    bottom=0.108,
)

LOGOS = {lab: plt.imread(str(HERE / f"logos/{lab}.png")) for lab in LAB_COLOR}
grad = np.linspace(0, 1, 256).reshape(-1, 1)


def draw_bars(ax, bar_rows, values, val_fmt, ymax, ytick_step, ylabel, errs=None):
    """Gradient bars + logo chips + shared axis cosmetics (panels 1-2)."""
    xs = list(range(len(bar_rows)))
    W = 0.62
    for x, v, r in zip(xs, values, bar_rows, strict=True):
        _, lab = NAME[r["model"]]
        c = LAB_COLOR[lab]
        cmap = LinearSegmentedColormap.from_list(
            f"g{id(ax)}{x}", [darken(c, 0.18), to_rgb(c), lighten(c, 0.42)]
        )
        clip = FancyBboxPatch(
            (x - W / 2, 0),
            W,
            v,
            boxstyle="round,pad=0,rounding_size=0.28",
            mutation_aspect=v / (ymax * 0.14),
            facecolor="none",
            edgecolor="none",
            transform=ax.transData,
        )
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
            fontsize=15,
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
    ax.set_yticks(range(0, int(ymax * 0.96) + 1, ytick_step))
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
fig.text(0.30, 0.9662, "Eval Suite 3 — Model Rankings", fontsize=29, color=MUTED, family=BRAND_MED)
fig.text(
    0.065,
    0.9424,
    "23 frontier-hard software-engineering tasks from real merged OSS PRs  ·  "
    "pass@1 across reasoning-effort levels  ·  Docker-sandboxed agent runs  ·  "
    "2026-08-01",
    fontsize=11.5,
    color=INK2,
    family=SANS,
)

# ---------------- Panel 1: pass@1 rankings ----------------
ax1 = fig.add_subplot(gs[0])
ax1.set_facecolor(SURFACE)
labels1 = []
for r in rows:
    disp, _ = NAME[r["model"]]
    partial = "*" if r["n_tasks"] < 23 else ""
    labels1.append(
        f"{disp} ({eff_display(r['model'], r['effort'])}){partial}"
        f"  ·  ${r['cost']:.2f}  ·  n={r['n_runs']}"
    )
draw_bars(
    ax1,
    rows,
    [r["pass1"] * 100 for r in rows],
    lambda v: f"{v:.0f}",
    108,
    20,
    "pass@1 (%)",
    errs=[(r.get("se") or 0) * 100 for r in rows],
)
ax1.set_xticklabels(
    labels1, rotation=42, ha="right", rotation_mode="anchor", fontsize=9.5, color=INK2, family=SANS
)

seen = dict.fromkeys(NAME[r["model"]][1] for r in rows)
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
    "Rankings by pass@1 — all effort levels",
    loc="left",
    fontsize=16,
    color=INK,
    pad=48,
    family=BRAND_MED,
)

# ---------------- Panel 2: minutes per task, fastest first ----------------
ax2 = fig.add_subplot(gs[1])
ax2.set_facecolor(SURFACE)
trows = sorted(rows, key=lambda r: r["avg_duration_s"] or 0)
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
    labels2, rotation=42, ha="right", rotation_mode="anchor", fontsize=9.5, color=INK2, family=SANS
)
ax2.set_title(
    "Speed — avg wall-clock minutes per task, fastest first",
    loc="left",
    fontsize=16,
    color=INK,
    pad=16,
    family=BRAND_MED,
)


# ---------------- Panel 3: effort-curve cards ----------------
def model_efforts(model: str) -> list[str]:
    """The provider's own effort ladder, low to high."""
    if model.startswith("deepseek:"):
        return ["low", "high", "extra-high"]  # DeepSeek: low/high/max
    if model.startswith("qwen:"):
        return ["low", "medium", "extra-high"]  # Qwen: low/medium/xhigh
    return ["low", "medium", "high"]


by_model: dict[str, dict[str, dict]] = {}
for r in rows:
    if r["effort"] in model_efforts(r["model"]):
        by_model.setdefault(r["model"], {})[r["effort"]] = r
swept = [m for m, effs in by_model.items() if len(effs) >= 2]
swept.sort(key=lambda m: -max(e["pass1"] for e in by_model[m].values()))

gs2 = gs[2].subgridspec(1, len(swept), wspace=0.16)
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
    axc = fig.add_subplot(gs2[k])
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
    axc.set_xlim(-0.42, 2.42)
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
    if k == 0:
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
    "Effort curves — how pass@1 responds to reasoning effort",
    fontsize=16,
    color=INK,
    family=BRAND_MED,
)

# ---------------- Footnote ----------------
fig.text(
    0.065,
    0.062,
    "* partial coverage — Claude Fable 5 excludes tasks refused by safety filters "
    "(low 19/23, medium 21/23, high 20/23); Kimi K3 19/23; Claude Haiku 4.5 21/23. "
    "Claude Opus 4.8 omitted (5/23 tasks).\n"
    "Claude Opus 5 columns are from vulcanbench.com Report 10 (single runs, "
    "2026-07-26); 4 of its 5 high-effort failures were wall-clock timeouts. "
    "Haiku 4.5 (default) and Kimi K3 (extra-high) have no effort sweep.\n"
    "DeepSeek's effort scale is low/high/max per its API; an accidental duplicate "
    "high run (its API coerces 'medium' to high) is excluded. Qwen's scale is "
    "low/medium/xhigh (no 'high').\n"
    "DeepSeek, Grok 4.5, and Qwen3.8-Max columns aggregate 3 runs/task (repeat "
    "sweeps); 5 Qwen xhigh runs on 3 tasks were lost to 600s API read timeouts "
    "and are excluded rather than scored 0.\n"
    "Whiskers are ±1 stderr — single-pass columns (n=23 runs or fewer) carry wider "
    "uncertainty than repeat-swept ones (n=52-71). Cost = total spend at list API "
    "prices across a column's runs. Time = sandbox wall-clock. "
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

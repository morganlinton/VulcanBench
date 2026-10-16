"""Muse Wonder Full beside Claude Opus 5.5, GPT-6.1 Sol and GPT-6 Astra.

One card on VulcanBench Frontier v4 at Low and Medium: the combined score, time
per task and cost per task, each as grouped bars labelled with their values. At the
owner's request the bars carry no whiskers; the standard errors stay in the
table and the CSV.

The combined score is 50% functional, 8.5% quality, 8.5% security and 33%
judged Code quality. Muse Wonder Full's comes from the v3.26 summary
(runs-code-quality-maintenance-v3.26, judged by Claude Opus 5.5 and, through
its v3.25 sibling, GPT-6.1 Sol); the other models' come from their published
tables, judged by Muse Spark 1.3 and Grok 4.6. Muse Spark is a Meta model and
is not neutral for a Meta submission, so the panels differ, and the card says so.
The functional score in the table is computed from each model's per-run rows,
counting every run.

Prices were checked against each vendor's official page on 2026-10-15
(docs/results/swe-v4-musewonder-full-2026-10/pricing-check.json):

- Muse Wonder Full: run summaries in runs-effort-musewonder-full-v2 (Muse Code
  1.4.6, Contributor mode, skill reminder off). Meta lists no API price for
  Wonder Full, so cost is an estimate from its token receipts at the Muse
  Wonder 1.0 list price ($4.00 input, $0.20 cached, $20.00 output, no
  long-context premium).
- Claude Opus 5.5: per-run functional scores from the public traces manifest
  (opus55-traces-manifest.csv, all 115 runs); cost and minutes from the
  frontier leaders card table, Claude Code's own total, which reprices exactly
  from the traces at today's list prices.
- GPT-6.1 Sol: per-run functional scores from its comparison.json; cost and
  minutes from the frontier leaders card table (Codex receipts at today's list
  price; long-context tier not applied, per-request sizes unrecorded).
- GPT-6 Astra: per-run functional scores from
  docs/results/swe-v4-astra-fable51-2026-09/comparison.json; cost and minutes
  from that report's economics table, plus its long-context upper bound.

Passed counts must match the published tables, or the card refuses to draw.
A Muse Wonder Full level with fewer than 23 tasks is marked preliminary, and a
level that mixes Muse Code versions is marked with a dagger. Medium's last two
tasks ran on Muse Code 1.4.9 in runs-effort-musewonder-full-v2-149, judged in
v3.26.1; 1.4.9 runs its goal and verify reminder observers on Muse Spark 1.3,
whose tokens are added to cost at Spark's list price (docs/DECISIONS.md,
2026-10-16).

    python scripts/cii-v4-board/make_musewonder_frontier_card.py
"""

from __future__ import annotations

import csv
import json
import math
import statistics
import sys
import textwrap
from collections import Counter
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch
from matplotlib.ticker import FixedLocator, FuncFormatter, NullFormatter

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import digest, save  # noqa: E402

RESULTS = ROOT / "docs/results"
OUTPUT = RESULTS / "swe-v4-musewonder-full-2026-10"
WONDER = ROOT / "runs-effort-musewonder-full-v2"
WONDER_149 = ROOT / "runs-effort-musewonder-full-v2-149"  # continues WONDER on Muse Code 1.4.9
WONDER_ROOTS = (WONDER, WONDER_149)
OPUS_SOL = RESULTS / "swe-v4-gpt6-vs-gpt56-2026-09/gpt61sol-opus55-astra-efforts.csv"
ASTRA_FABLE = RESULTS / "swe-v4-astra-fable51-2026-09/astra-vs-fable51-v34-economics-efforts.csv"
OPUS_MANIFEST = OUTPUT / "opus55-traces-manifest.csv"
SOL_ROWS = RESULTS / "swe-v4-gpt61-sol-2026-09/comparison.json"
AF_ROWS = RESULTS / "swe-v4-astra-fable51-2026-09/comparison.json"
PRICING = OUTPUT / "pricing-check.json"
JUDGING = ROOT / "runs-code-quality-maintenance-v3.26"
JUDGING_PROTOCOL = "code-quality-maintenance-v3.26"
TOPUP = ROOT / "runs-code-quality-maintenance-v3.26.1"
TOPUP_PROTOCOL = "code-quality-maintenance-v3.26.1"
PANELS = {
    "wonder": "Claude Opus 5.5 and GPT-6.1 Sol (v3.26, v3.26.1)",
    "opus55": "Muse Spark 1.3 and Grok 4.6 (v3.15)",
    "gpt61sol": "Muse Spark 1.3 and Grok 4.6 (v3.18)",
    "astra": "Muse Spark 1.3 and Grok 4.6 (v3.4)",
}
LEVELS = ("low", "medium", "high", "extra-high", "max")
TABLE_LEVELS = ("low", "medium")
SUITE_TASKS = 23
MODELS = ("wonder", "opus55", "gpt61sol", "astra")
NAMES = {
    "wonder": "Muse Wonder Full",
    "opus55": "Claude Opus 5.5",
    "gpt61sol": "GPT-6.1 Sol",
    "astra": "GPT-6 Astra",
}
HARNESS = {
    "wonder": "Muse Code 1.4.6",
    "opus55": "Claude Code 2.1.280",
    "gpt61sol": "Codex 0.159.0",
    "astra": "Codex",
}
# Lab colors (CLAUDE.md): Meta blue; Anthropic clay plus a darker clay for the
# second Anthropic model; OpenAI greens as on the frontier leaders card.
COLORS = {
    "wonder": "#0866FF",
    "opus55": "#D97757",
    "gpt61sol": "#0B3D2E",
    "astra": "#10A37F",
}
MARKERS = {"wonder": "o", "opus55": "s", "gpt61sol": "^", "astra": "D"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
SPARK_RATES = (1.25, 0.15, 4.25)  # Muse Spark 1.3 Standard list price, Meta page 2026-10-16
WONDER_RATES = (
    4.00,
    0.20,
    20.00,
)  # Muse Wonder 1.0 list price per million: input, cached input, output


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return statistics.mean(values), se


def level_group(func, usd, minutes, passed_published=None, model="", level=""):
    require(len(func) == SUITE_TASKS or model == "wonder", f"{model} {level}: {len(func)} runs")
    m, se = mean_se(func)
    passed = sum(f >= 1 for f in func)
    if passed_published is not None:
        require(
            passed == passed_published,
            f"{model} {level}: passed {passed} vs published {passed_published}",
        )
    return {
        "n": len(func),
        "functional": 100 * m,
        "functional_se": 100 * se,
        "passed": passed,
        "usd": usd,
        "minutes": minutes,
    }


def reminder_usd(run_dir):
    """Spark reminder-observer tokens a Muse Code 1.4.9 run recorded apart, at Spark's list price."""
    trace = run_dir / "trace.jsonl"
    if not trace.exists():
        return 0.0
    for line in trace.read_text().splitlines():
        event = json.loads(line)
        if event.get("type") == "muse_usage":
            usd = 0.0
            for model, u in ((event.get("data") or {}).get("auxiliary") or {}).items():
                require(model == "muse-spark-1.3", f"unexpected reminder model {model}")
                rin, rcached, rout = SPARK_RATES
                cached = u.get("cached_tokens", 0)
                usd += (
                    (u["input_tokens"] - cached) * rin
                    + cached * rcached
                    + u["output_tokens"] * rout
                ) / 1e6
            return usd
    return 0.0


def wonder_summaries(level):
    """(run folder, summary) for every finished run at this level across both Muse Code roots."""
    return [
        (p.parent, json.loads(p.read_text()))
        for root in WONDER_ROOTS
        for p in sorted((root / level).glob("*/summary.json"))
    ]


def load_wonder():
    groups = {}
    for level in TABLE_LEVELS:  # the judged levels; High and Extra-high join once judged
        found = wonder_summaries(level)
        runs = [r for _, r in found]
        if not runs:
            continue
        require(
            len({r["task_id"] for r in runs}) == len(runs),
            f"duplicate Muse Wonder Full tasks at {level}",
        )
        usd = []
        for run_dir, r in found:
            require(
                r["model"] == "muse-code:muse-wonder-full-1.0-contributor", f"model {r['model']}"
            )
            require(r["effort"]["requested"] == level, f"effort {r['effort']}")
            cd = r["cost_detail"]
            output = r["total_tokens"] - cd["agent_input_tokens"]
            rin, rcached, rout = WONDER_RATES
            usd.append(
                (
                    cd["agent_uncached_input_tokens"] * rin
                    + cd["agent_cached_input_tokens"] * rcached
                    + output * rout
                )
                / 1e6
                + reminder_usd(run_dir)
            )
        groups[level] = level_group(
            [r["scores"]["functional"] for r in runs],
            statistics.mean(usd),
            statistics.mean(r["duration_s"] / 60 for r in runs),
            model="wonder",
            level=level,
        )
        groups[level]["versions"] = dict(Counter(r["cli_agent"]["harness_version"] for r in runs))
    return groups


def load_wonder_judged(groups):
    """Combined score per level from the frozen v3.26 summary and its v3.26.1 top-up.

    Both rounds must be ready for publication with both seats passing; each
    level's combined score is the mean over every judged run in either.
    """
    rows, hashes = [], {}
    for root, protocol_id in ((JUDGING, JUDGING_PROTOCOL), (TOPUP, TOPUP_PROTOCOL)):
        if root == TOPUP and not (root / "summary.json").exists():
            continue
        summary = json.loads((root / "summary.json").read_text())
        require(summary["protocol"] == protocol_id, f"wrong judging protocol in {root.name}")
        require(summary["ready_for_publication"], f"{protocol_id} is not ready for publication")
        require(set(summary["passing_panels"]) == {"claude", "sol"}, f"{protocol_id} panels")
        rows += [r for r in summary["rows"] if r.get("published")]
        hashes[protocol_id] = {
            p.name: digest(p.read_bytes()) for p in (root / "summary.json", root / "protocol.json")
        }
    for lvl, g in groups.items():
        level_rows = [r for r in rows if r["effort"] == lvl]
        require(
            len({r["task"] for r in level_rows}) == len(level_rows), f"{lvl}: a task judged twice"
        )
        require(len(level_rows) == g["n"], f"{lvl}: {len(level_rows)} judged vs {g['n']} runs")
        g["combined"], g["combined_se"] = mean_se(
            [r["published"]["composite_v3"] for r in level_rows]
        )
        g["by_judge_l1"] = {
            p: statistics.mean(r["panels"][p]["l1"]["score"] for r in level_rows)
            for p in ("claude", "sol")
        }
    return hashes


def load_others():
    pub, combined = {}, {}
    with OPUS_SOL.open() as fh:
        for r in csv.DictReader(fh):
            if r["model"] in ("opus55", "gpt61sol"):
                pub[r["model"], r["effort"]] = (
                    float(r["mean_usd"]),
                    float(r["mean_minutes"]),
                    int(r["passed"]),
                    None,
                )
            if r["model"] in ("opus55", "gpt61sol", "astra"):
                combined[r["model"], r["effort"]] = (float(r["combined"]), float(r["combined_se"]))
    with ASTRA_FABLE.open() as fh:
        for r in csv.DictReader(fh):
            upper = float(r["long_context_upper_mean_usd"]) if r["model"] == "astra" else None
            pub[r["model"], r["effort"]] = (
                float(r["mean_usd"]),
                float(r["mean_minutes"]),
                None,
                upper,
            )
    func = {m: {lvl: [] for lvl in LEVELS} for m in MODELS[1:]}
    with OPUS_MANIFEST.open() as fh:
        for r in csv.DictReader(fh):
            func["opus55"][r["effort"]].append(float(r["functional"]))
    for r in json.loads(SOL_ROWS.read_text())["rows"]:
        func["gpt61sol"][r["effort"]].append(r["functional"])
    for r in json.loads(AF_ROWS.read_text())["rows"]:
        if r["model"] in func:
            func[r["model"]][r["effort"]].append(r["functional"])
    groups = {}
    for m in MODELS[1:]:
        groups[m] = {}
        for lvl in LEVELS:
            usd, minutes, passed, upper = pub[m, lvl]
            groups[m][lvl] = level_group(func[m][lvl], usd, minutes, passed, m, lvl)
            groups[m][lvl]["usd_upper"] = upper
            if (m, lvl) in combined:
                groups[m][lvl]["combined"], groups[m][lvl]["combined_se"] = combined[m, lvl]
    return groups


def main():  # noqa: PLR0915, one linear figure
    groups = {"wonder": load_wonder(), **load_others()}
    judging_hashes = load_wonder_judged(groups["wonder"])
    wonder_levels = [lvl for lvl in LEVELS if lvl in groups["wonder"]]
    require(
        wonder_levels and wonder_levels == list(LEVELS[: len(wonder_levels)]),
        "Muse Wonder Full levels must start at Low",
    )
    prelim = {lvl for lvl in wonder_levels if groups["wonder"][lvl]["n"] < SUITE_TASKS}
    final = not prelim and len(wonder_levels) == len(LEVELS)
    mixed = {lvl for lvl in wonder_levels if len(groups["wonder"][lvl]["versions"]) > 1}

    def level_mark(lvl):
        """* for a preliminary level, a dagger for a level that mixes Muse Code versions."""
        return ("*" if lvl in prelim else "") + ("\u2020" if lvl in mixed else "")

    wonder_harness = (
        "Muse Code "
        + ", ".join(
            sorted(
                {
                    v.split(" (")[0].removeprefix("Muse Code ")
                    for g in groups["wonder"].values()
                    for v in g["versions"]
                }
            )
        )
        + ("\u2020" if mixed else "")
    )
    pricing = json.loads(PRICING.read_text())

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 15.4
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, numeric=False, ha="left", color=INK):
        s = s.replace("$", r"\$")  # plain dollar signs, not mathtext
        fam = (
            "IBM Plex Mono"
            if numeric
            else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium")
            if heading
            else "Geist"
        )
        wt = (
            (500 if bold else 400)
            if numeric
            else ((600 if bold else 500) if heading else (700 if bold else 400))
        )
        return fig.text(
            x, yf(y), s, fontsize=size, fontfamily=fam, weight=wt, ha=ha, va="center", color=color
        )

    def line(x1, x2, y, color=RULE, width=0.8):
        fig.add_artist(
            plt.Line2D([x1, x2], [yf(y), yf(y)], transform=fig.transFigure, color=color, lw=width)
        )

    def swatch(x, y, m, size=9):
        fig.add_artist(
            plt.Line2D(
                [x],
                [yf(y)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=size,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )

    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / W, 0.42 / H])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(right, 0.65, "October 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    text(
        left,
        1.72,
        "VulcanBench Frontier v4: Muse Wonder Full beside the frontier",
        30,
        True,
        heading=True,
    )
    head = " and ".join(
        f"{groups['wonder'][lvl]['combined']:.1f} at {lvl.capitalize()}{level_mark(lvl)} "
        f"({groups['wonder'][lvl]['minutes']:.0f} min, ~${groups['wonder'][lvl]['usd']:.2f} per task)"
        for lvl in wonder_levels
    )
    text(left, 2.22, f"Muse Wonder Full scores {head}.", 15)
    text(
        left,
        2.60,
        "Combined score: 50% functional, 8.5% quality, 8.5% security, 33% judged Code quality. "
        "Muse Wonder Full was judged by a different panel (see notes).",
        13.5,
        color=MUTED,
    )
    x = left
    for m in MODELS:
        swatch(x + 0.004, 3.05, m)
        text(x + 0.016, 3.05, NAMES[m], 12.5, True)
        x += 0.22

    chart_levels = TABLE_LEVELS
    xs = list(range(len(chart_levels)))
    width = 0.19
    offsets = {m: (i - (len(MODELS) - 1) / 2) * width for i, m in enumerate(MODELS)}

    def axis(x0, w, top, h):
        ax = fig.add_axes([x0, yf(top + h), w, h / H], facecolor=PAPER)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="x", length=0, labelsize=13, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        ax.set_xticks(xs, [lvl.capitalize() + level_mark(lvl) for lvl in chart_levels])
        ax.set_xlim(-0.5, len(chart_levels) - 0.5)
        return ax

    def bars(ax, key, fmt, hatched):
        """Grouped bars for Low and Medium, each labelled with its value; no whiskers."""
        for m in MODELS:
            bx = [i + offsets[m] for i in xs]
            ys = [groups[m][lvl][key] for lvl in chart_levels]
            drawn = ax.bar(
                bx, ys, width=width * 0.9, color=COLORS[m], edgecolor=INK, linewidth=0.5, zorder=2
            )
            for patch, xx, y_, lvl in zip(drawn, bx, ys, chart_levels, strict=True):
                if hatched(m, lvl):
                    patch.set_hatch("///")
                ax.annotate(
                    fmt(y_),
                    (xx, y_),
                    xytext=(0, 4),
                    textcoords="offset points",
                    ha="center",
                    va="bottom",
                    fontsize=8.5,
                    fontfamily="IBM Plex Mono",
                    color=INK,
                )
                if m == "wonder" and lvl in prelim:
                    ax.annotate(
                        f"n={groups[m][lvl]['n']}",
                        (xx, ax.get_ylim()[0]),
                        xytext=(0, 6),
                        textcoords="offset points",
                        ha="center",
                        va="bottom",
                        rotation=90,
                        fontsize=9,
                        fontfamily="IBM Plex Mono",
                        color="#ffffff",
                    )

    # Top row: combined score, time per task and cost per task, grouped bars at Low and Medium.
    prelim_bar = lambda m, lvl: m == "wonder" and lvl in prelim  # noqa: E731
    panel_x = (left, 0.37, 0.68)
    panel_w = 0.24

    def panel(i, title, subtitle):
        text(panel_x[i], 3.62, title, 20, True, heading=True)
        text(panel_x[i], 3.94, subtitle, 11, color=MUTED)
        return axis(panel_x[i] + 0.022, panel_w, 4.25, 3.3)

    hatch_note = "  ·  hatched = preliminary" if prelim else ""
    ax = panel(0, "Combined score", "higher is better  ·  focused scale" + hatch_note)
    lo = min(groups[m][lvl]["combined"] for m in MODELS for lvl in chart_levels)
    hi = max(groups[m][lvl]["combined"] for m in MODELS for lvl in chart_levels)
    ax.set_ylim(math.floor(lo / 2) * 2 - 2, math.ceil(hi / 2) * 2 + 1.5)
    bars(ax, "combined", lambda v: f"{v:.1f}", prelim_bar)

    ax = panel(1, "Time per task", "minutes  ·  lower is faster" + hatch_note)
    top = max(groups[m][lvl]["minutes"] for m in MODELS for lvl in chart_levels)
    ax.set_ylim(0, math.ceil(top * 1.15 / 5) * 5)
    bars(ax, "minutes", lambda v: f"{v:.1f}", prelim_bar)

    ax = panel(2, "Cost per task", "USD at list prices  ·  log scale  ·  hatched = estimate")
    ax.set_yscale("log")
    vals = [groups[m][lvl]["usd"] for m in MODELS for lvl in chart_levels]
    ax.set_ylim(min(vals) / 1.8, max(vals) * 2.2)
    ax.yaxis.set_major_locator(
        FixedLocator(
            [t for t in (0.25, 0.5, 1, 2, 4, 8) if ax.get_ylim()[0] <= t <= ax.get_ylim()[1]]
        )
    )
    ax.yaxis.set_major_formatter(FuncFormatter(lambda v, _: f"${v:g}"))
    ax.yaxis.set_minor_formatter(NullFormatter())
    bars(ax, "usd", lambda v: f"{v:.2f}", lambda m, lvl: m == "wonder")

    # Table: the levels Muse Wonder Full has run.
    ty = 8.25

    def star(lvl):
        return level_mark(lvl)

    text(
        left,
        ty,
        "Table 1  |  The levels Muse Wonder Full has run",
        14.5,
        heading=True,
    )
    cols = {
        ("low", "f"): 0.43,
        ("low", "p"): 0.50,
        ("low", "usd"): 0.57,
        ("low", "min"): 0.64,
        ("medium", "f"): 0.73,
        ("medium", "p"): 0.80,
        ("medium", "usd"): 0.87,
        ("medium", "min"): 0.94,
    }
    line(left, right, ty + 0.28, INK, 1.2)
    text(0.535, ty + 0.50, "Low" + star("low"), 12.5, True, ha="center")
    text(0.835, ty + 0.50, "Medium" + star("medium"), 12.5, True, ha="center")
    line(0.39, 0.645, ty + 0.66, RULE, 0.6)
    line(0.69, right, ty + 0.66, RULE, 0.6)
    text(left, ty + 0.86, "Model  ·  harness", 12, True)
    for lvl in TABLE_LEVELS:
        for k, lab in (
            ("f", "Combined ±se"),
            ("p", "Functional"),
            ("usd", "$/task"),
            ("min", "Min"),
        ):
            text(cols[lvl, k], ty + 0.86, lab, 11, True, ha="right", color=MUTED)
    line(left, right, ty + 1.04, INK, 0.6)
    y = ty + 1.32
    for m in MODELS:
        swatch(left + 0.004, y, m, 7)
        text(left + 0.016, y, NAMES[m], 12.5, True)
        text(left + 0.131, y, wonder_harness if m == "wonder" else HARNESS[m], 11, color=MUTED)
        for lvl in TABLE_LEVELS:
            g = groups[m][lvl]
            usd = (
                f"~{g['usd']:.2f}"
                if m == "wonder"
                else f"{g['usd']:.2f}" + ("+" if g.get("usd_upper") else "")
            )
            text(
                cols[lvl, "f"],
                y,
                f"{g['combined']:.1f} ±{g['combined_se']:.1f}",
                12.5,
                numeric=True,
                ha="right",
            )
            text(cols[lvl, "p"], y, f"{g['functional']:.1f}", 12.5, numeric=True, ha="right")
            text(cols[lvl, "usd"], y, usd, 12.5, numeric=True, ha="right")
            text(cols[lvl, "min"], y, f"{g['minutes']:.1f}", 12.5, numeric=True, ha="right")
        y += 0.40
        line(left, right, y - 0.2, RULE, 0.6)
    line(left, right, y - 0.2, INK, 1.2)

    wm = groups["wonder"].get("medium")
    notes = [
        (
            f"* Muse Wonder Full Medium covers {wm['n']} of {SUITE_TASKS} tasks so far (hatched combined and time bars; every other bar is n=23); "
            "the last two run after its Muse Code weekly limit resets."
            if wm and "medium" in prelim
            else "\u2020 Muse Wonder Full Medium covers all 23 tasks: 21 ran on Muse Code 1.4.6 and 2 (cellarcore, paddockcore) on Muse Code "
            "1.4.9 after the quota reset, judged by the same panel in v3.26.1. Muse Code 1.4.9 runs its goal and verify reminder observers "
            "on Muse Spark 1.3, not Muse Wonder Full; their tokens are added to cost at Spark's list price ($1.25 input, $0.15 cached, "
            "$4.25 output per million)."
            if "medium" in mixed
            else "Muse Wonder Full levels cover all 23 tasks."
        ),
        "Combined score = 50% functional (share of hidden behaviour tests passed) + 8.5% quality + 8.5% security + 33% Code quality judged by "
        "two calibrated model judges; the table gives its standard error (±se) and the functional score. Code quality judges (model: panel): "
        + "; ".join(f"{NAMES[m]}: {PANELS[m]}" for m in MODELS)
        + ". Muse Spark 1.3 is a Meta model and is not neutral for a Meta submission, so Muse Wonder Full has a different panel; "
        "scores from different panels are not calibrated against each other. Claude Opus 5.5 judged Muse Wonder Full and is also "
        "compared here; its own column is from its own round's panel. The two Muse Wonder Full judges differed: reviewed Code quality "
        + " and ".join(
            f"{groups['wonder'][lvl]['by_judge_l1']['claude']:.0f} from Opus 5.5 and {groups['wonder'][lvl]['by_judge_l1']['sol']:.0f} "
            f"from Sol at {lvl.capitalize()}"
            for lvl in wonder_levels
        )
        + ", out of 100.",
        f"Prices verified on {pricing['checked_at']} against each vendor's official pricing page. Muse Wonder Full ran on a Muse Code "
        "subscription and Meta lists no API price for it, so its cost (~, hatched) is an estimate from its token use at the Muse Wonder 1.0 "
        "list price ($4.00 input, $0.20 cached, $20.00 output per million, no long-context premium). Claude Opus 5.5 is Claude Code's own "
        "total, which reprices exactly from its traces at today's list prices ($4.00 input, $0.20 cache hit, $8.00 one-hour cache write, "
        "$20.00 output; fallback turns at their own model's rate). GPT-6.1 Sol is its Codex token receipts at $2.00 / $0.10 / $10.00. "
        "GPT-6 Astra ($10 / $1 / $50) is its report's central estimate at that price.",
        "GPT-6 Astra and GPT-6.1 Sol charge 2x input and 1.5x output on requests over 272K input tokens, and per-request sizes were not "
        "recorded: + in the table marks Astra, whose report gives an upper bound of $3.28 at Low and $2.79 at Medium; Sol's calls average about 35K input tokens, "
        "so its premium is likely small and is not applied.",
        "Muse Wonder Full ran in Contributor mode with Muse Code's skill reminder off (13 to 16 October 2026); 9 of its 23 Medium "
        "tasks overlapped another local job, which can affect minutes but not scores. Other models are their published Frontier v4 runs, "
        "not re-run. Harnesses differ.",
    ]
    yy = y + 0.12
    for n_ in notes:
        for w in textwrap.wrap(n_, 205):
            text(left, yy, w, 10.3, color=MUTED)
            yy += 0.235
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"musewonder-full-vs-frontier{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"musewonder-full-vs-frontier{suffix}-efforts.csv"
    basis = {
        "wonder": "estimate at Muse Wonder 1.0 list price",
        "opus55": "Claude Code total, reprices exactly at list price",
        "gpt61sol": "Codex receipts at list price, long-context tier not applied",
        "astra": "report central estimate at list price; upper bound in usd_upper",
    }
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "model",
                "effort",
                "n",
                "combined",
                "combined_se",
                "code_quality_panel",
                "functional_pct",
                "functional_se_pct",
                "passed",
                "usd_per_task",
                "usd_upper",
                "minutes",
                "cost_basis",
            ]
        )
        for m in MODELS:
            for lvl in LEVELS:
                if lvl not in groups[m]:
                    continue
                g = groups[m][lvl]
                w.writerow(
                    [
                        m,
                        lvl,
                        g["n"],
                        f"{g['combined']:.4f}" if "combined" in g else "",
                        f"{g['combined_se']:.4f}" if "combined" in g else "",
                        PANELS[m],
                        f"{g['functional']:.2f}",
                        f"{g['functional_se']:.2f}",
                        g["passed"],
                        f"{g['usd']:.4f}",
                        f"{g['usd_upper']:.4f}" if g.get("usd_upper") else "",
                        f"{g['minutes']:.2f}",
                        basis[m],
                    ]
                )
    sources = {
        p.name: digest(p.read_bytes())
        for p in (OPUS_SOL, ASTRA_FABLE, OPUS_MANIFEST, SOL_ROWS, AF_ROWS, PRICING)
    }
    sources["wonder_judging"] = judging_hashes
    sources["opus55_traces_repo"] = "https://github.com/morganlinton/vulcanbench-opus55-traces"
    sources["wonder_summaries"] = {
        f"{root.name}/{lvl}/{p.parent.name}": digest(p.read_bytes())
        for root in WONDER_ROOTS
        for lvl in wonder_levels
        for p in sorted((root / lvl).glob("*/summary.json"))
    }
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "groups": groups,
            "sources": sources,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in MODELS:
        print(
            f"  {NAMES[m]:<18}"
            + "  ".join(
                f"{lvl[:3]} {g.get('combined', float('nan')):.1f} ({g['functional']:.1f}%) ${g['usd']:.2f}"
                for lvl, g in groups[m].items()
            )
        )


if __name__ == "__main__":
    main()

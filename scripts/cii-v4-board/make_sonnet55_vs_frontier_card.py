"""Claude Sonnet 5.5, Claude Opus 5.5 and GPT-6.1 Sol on VulcanBench Frontier v4.

One card: combined score and cost per task at every effort level, and a table
with tasks passed, cost and the refusal-fallback share for each column.

Sources, nothing re-judged:

- Claude Opus 5.5: the published per-level rows in
  docs/results/swe-v4-opus55-2026-09/opus55-astra-fable-efforts.csv (Code
  quality v3.15).
- GPT-6.1 Sol: the published per-level rows in
  docs/results/swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-efforts.csv (Code
  quality v3.18) and its API-equivalent cost in
  gpt61-sol-v318-economics-efforts.csv.
- The raw judging directories behind both were lost with the original host,
  so the committed tables are the source of record.
- Claude Sonnet 5.5: the Code quality v3.23 summary and manifest, aggregated by
  make_sonnet55_v323_card.py (cost is Claude Code's own list-price total).

    python scripts/cii-v4-board/make_sonnet55_vs_frontier_card.py
"""

from __future__ import annotations

import csv
import importlib.util
import math
import statistics
import sys
import textwrap
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

PUBLISHED = ROOT / "docs/results/swe-v4-opus55-2026-09/opus55-astra-fable-efforts.csv"
SOL = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-efforts.csv"
SOL_COST = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-economics-efforts.csv"
OUTPUT = ROOT / "docs/results/swe-v4-sonnet55-2026-10"
MODELS = ("sonnet55", "opus55", "gpt61sol")
NAMES = {"sonnet55": "Claude Sonnet 5.5", "opus55": "Claude Opus 5.5", "gpt61sol": "GPT-6.1 Sol"}
HARNESS = {
    "sonnet55": "Claude Code 2.1.291 to 2.1.293",
    "opus55": "Claude Code 2.1.280",
    "gpt61sol": "Codex 0.159.0",
}
# Sonnet 5.5 keeps Anthropic clay, as on its own card; Opus 5.5 keeps its
# comparison-card hue; GPT-6.1 Sol takes the OpenAI lab color (CLAUDE.md).
# Markers, direct labels and the table carry identity as well as color.
COLORS = {"sonnet55": "#D97757", "opus55": "#8C3A1F", "gpt61sol": "#10A37F"}
MARKERS = {"sonnet55": "o", "opus55": "D", "gpt61sol": "^"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CLAUDE_MAIN = {
    "sonnet55": "claude-sonnet-5-5",
    "opus55": "claude-opus-5-5",
}


def _sonnet_card():
    spec = importlib.util.spec_from_file_location(
        "sonnet55_card", Path(__file__).with_name("make_sonnet55_v323_card.py")
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = [v for v in values if v is not None]
    if not values:
        return {"n": 0, "mean": None, "se": None}
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return {"n": len(values), "mean": statistics.mean(values), "se": se}


def load_groups():
    """Per-level aggregates: Opus 5.5 and GPT-6.1 Sol from published tables, Sonnet 5.5 from v3.23."""
    groups = {}
    with PUBLISHED.open() as handle:
        for r in csv.DictReader(handle):
            if r["model"] != "opus55":
                continue
            groups[r["model"], r["effort"]] = {
                "n": int(r["n"]),
                "combined": {
                    "n": int(r["judged"]),
                    "mean": float(r["combined"]),
                    "se": float(r["combined_se"]),
                },
                "code_quality": {"mean": float(r["code_quality"])},
                "passed": int(r["passed"]),
                "usd": {"mean": float(r["usd_per_task"])},
                "minutes": {"mean": float(r["minutes"])},
                "fb_runs": int(r["fallback_runs"]),
                "fb_share": float(r["opus48_reply_share_pct"]),
            }
    with SOL_COST.open() as handle:
        sol_cost = {r["effort"]: r for r in csv.DictReader(handle)}
    with SOL.open() as handle:
        for r in csv.DictReader(handle):
            require(r["model"] == "gpt61sol", f"unexpected row {r['model']}")
            c = sol_cost[r["effort"]]
            groups["gpt61sol", r["effort"]] = {
                "n": int(r["n"]),
                "combined": {
                    "n": int(r["n"]),
                    "mean": float(r["combined_v3"]),
                    "se": float(r["combined_v3_se"]),
                },
                "code_quality": {"mean": float(r["code_quality"])},
                "passed": int(r["passed"]),
                "usd": {"mean": float(c["mean_usd"])},
                "minutes": {"mean": float(r["minutes"])},
                "fb_runs": 0,
                "fb_share": None,
            }
    card = SONNET
    _summary, _protocol, rows, final, _coverage = card.load()
    for (_m, e), g in card.aggregate(rows).items():
        groups["sonnet55", e] = {
            "n": 23,
            "combined": g["combined"],
            "code_quality": g["code_quality"],
            "passed": g["passed"],
            "usd": g["cli_usd"],
            "minutes": g["minutes"],
            "fb_runs": g["fallbacks"],
            "fb_share": 100
            * sum(
                sum(
                    v
                    for k, v in r["replies"].items()
                    if k not in (CLAUDE_MAIN["sonnet55"], "<synthetic>")
                )
                for r in rows
                if r["effort"] == e
            )
            / max(1, sum(sum(r["replies"].values()) for r in rows if r["effort"] == e)),
        }
    for m in MODELS:
        for e in LEVELS:
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "published_tables": {p.name: digest(p.read_bytes()) for p in (PUBLISHED, SOL, SOL_COST)},
        "v3.23_summary_sha256": digest((card.RUN / "summary.json").read_bytes()),
    }
    return groups, final, sources


SONNET = _sonnet_card()


def main():  # noqa: PLR0912, PLR0915, one linear figure
    groups, final, sources = load_groups()

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 16.3
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, numeric=False, ha="left", color=INK):
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

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(
        left,
        1.72,
        "VulcanBench Frontier v4: Sonnet 5.5, Opus 5.5 and GPT-6.1 Sol",
        30,
        True,
        heading=True,
    )
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])
    head = "  ·  ".join(
        f"{NAMES[m]} {groups[m, best[m]]['combined']['mean']:.2f} at {best[m].replace('-', ' ')}"
        for m in order
    )
    text(left, 2.22, f"Best combined score per model: {head}.", 15)
    text(
        left,
        2.60,
        "23 hard legacy-reconstruction tasks per effort level. Code quality judged by Muse Spark 1.3 and Grok 4.6.",
        13.5,
        color=MUTED,
    )

    x = left
    for m in MODELS:
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(3.05)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=9,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        label = f"{NAMES[m]}{' (with fallback)' if m in CLAUDE_MAIN else ''}"
        text(x + 0.016, 3.05, label, 13, True)
        x += {"sonnet55": 0.30, "opus55": 0.29, "gpt61sol": 0}[m]

    def axis(x0, w, top, h):
        ax = fig.add_axes([x0, yf(top + h), w, h / H], facecolor=PAPER)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
        return ax

    text(left, 3.62, "Combined score", 20, True, heading=True)
    text(
        left,
        3.94,
        "/100  ·  higher is better  ·  focused scale  ·  whiskers +/-1 se",
        12,
        color=MUTED,
    )
    ax = axis(0.085, 0.40, 4.25, 3.2)
    vals = [groups[m, e]["combined"] for m in MODELS for e in LEVELS]
    lo = math.floor(min(v["mean"] - v["se"] for v in vals)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 1
    # End labels: keep at least ~6.5% of the axis between neighbours so close finishes stay legible.
    gap = 0.065 * (hi - lo)
    label_y = {}
    for m in sorted(MODELS, key=lambda m: groups[m, "max"]["combined"]["mean"]):
        y_end = groups[m, "max"]["combined"]["mean"]
        floor = max(label_y.values(), default=-math.inf) + gap
        label_y[m] = max(y_end, floor)
    ax.set_ylim(lo, hi)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    for m in MODELS:
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        ax.plot(range(5), ys, color=COLORS[m], lw=2, zorder=2)
        ax.errorbar(
            range(5),
            ys,
            yerr=es,
            fmt=MARKERS[m],
            color=COLORS[m],
            markersize=8,
            markeredgecolor=INK,
            markeredgewidth=0.6,
            ecolor=INK,
            elinewidth=0.8,
            capsize=3,
            zorder=3,
        )
        ax.annotate(
            NAMES[m].replace("Claude ", ""),
            (4, ys[-1]),
            xytext=(4.12, label_y[m]),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
        )

    text(0.56, 3.62, "Cost per task", 20, True, heading=True)
    text(
        0.56,
        3.94,
        "USD, API-equivalent at list prices  ·  every serving model included  ·  bases in notes",
        12,
        color=MUTED,
    )
    ax = axis(0.585, 0.355, 4.25, 3.2)
    shifts = {"sonnet55": -0.26, "opus55": 0.0, "gpt61sol": 0.26}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(5)],
            ys,
            width=0.25,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in enumerate(ys):
            ax.annotate(
                f"{y:.1f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    ty = 8.35
    text(left, ty, "Table 1  |  Every effort level", 14.5, heading=True)
    cols = dict(zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True))
    line(left, right, ty + 0.28, INK, 1.2)
    text(left, ty + 0.52, "Model and measure", 12.5, True)
    for e in LEVELS:
        text(cols[e], ty + 0.52, e.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, ty + 0.72, INK, 0.6)
    y = ty + 0.98
    for m in MODELS:
        text(left, y, NAMES[m], 13, True)
        fig.add_artist(
            plt.Line2D(
                [left - 0.012],
                [yf(y)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=7,
                markeredgecolor=INK,
                markeredgewidth=0.5,
                linestyle="none",
            )
        )
        measures = [
            (
                "Combined score",
                lambda g: f"{g['combined']['mean']:.2f}" + ("*" if g["combined"]["n"] < 23 else ""),
                True,
            ),
            ("Tasks passed of 23", lambda g: str(g["passed"]), False),
            ("Cost per task, USD", lambda g: f"{g['usd']['mean']:.2f}", False),
        ]
        if m in CLAUDE_MAIN:
            measures.append(
                (
                    "Fallback share of replies (runs)",
                    lambda g: f"{g['fb_share']:.1f}% ({g['fb_runs']})",
                    False,
                )
            )
        for label, fn, strong in measures:
            y += 0.30
            text(left + 0.012, y, label, 12, color=INK if strong else MUTED)
            for e in LEVELS:
                g = groups[m, e]
                emph = strong and e == best[m]
                text(
                    cols[e],
                    y,
                    fn(g),
                    13 if strong else 12,
                    emph,
                    numeric=True,
                    ha="right",
                    color=INK,
                )
        y += 0.40
        line(left, right, y - 0.2, RULE, 0.6)
    line(left, right, y - 0.2, INK, 1.2)

    notes = [
        "* Combined score over the judged runs when fewer than 23. Bold marks each model's best level.",
        "Protocols: Sonnet 5.5 is Code quality v3.23, Opus 5.5 is v3.15 and GPT-6.1 Sol is v3.18. Same rubric, controls, weights and "
        "judges (Muse Spark 1.3 and Grok 4.6, neutral for both labs), judged in separate sessions. For v3.23 the judge settings were checked against the original protocols, recovered from a private "
        "backup: identical. Muse ran the same binary (sha256 match); Grok ran through Cursor CLI 2026.10.01, "
        "which updates itself (the v3.3 round recorded 2026.09.02).",
        "Cost bases differ: the Claude columns use Claude Code's own list-price total per task; GPT-6.1 Sol is API-equivalent at "
        "list prices from its token ledger (its runs used the ChatGPT Pro subscription). Both Claude columns ran with Claude Code's "
        "refusal fallback on and count every run; Sonnet 5.5 never fell back. Codex has no refusal fallback.",
        "Harnesses differ (Claude Code 2.1.291 to 2.1.293, Claude Code 2.1.280, Codex 0.159.0), so small gaps are harness "
        "confounded. Opus 5.5 and GPT-6.1 Sol numbers are their published per-level rows; nothing was re-run or re-judged.",
    ]
    yy = y + 0.12
    for n in notes:
        for w in textwrap.wrap(n, 200):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"sonnet55-vs-frontier{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"sonnet55-vs-frontier{suffix}-efforts.csv"
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "model",
                "effort",
                "n",
                "judged",
                "combined",
                "combined_se",
                "code_quality",
                "passed",
                "usd_per_task",
                "minutes",
                "opus48_reply_share_pct",
                "fallback_runs",
            ]
        )
        for (m, e), g in groups.items():
            w.writerow(
                [
                    m,
                    e,
                    g["n"],
                    g["combined"]["n"],
                    f"{g['combined']['mean']:.4f}",
                    f"{g['combined']['se']:.4f}",
                    f"{g['code_quality']['mean']:.4f}",
                    g["passed"],
                    f"{g['usd']['mean']:.4f}",
                    f"{g['minutes']['mean']:.4f}",
                    "" if g["fb_share"] is None else f"{g['fb_share']:.2f}",
                    g["fb_runs"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "sources": sources,
            "best": best,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

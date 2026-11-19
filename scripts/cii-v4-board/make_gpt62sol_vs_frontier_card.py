"""GPT-6.2 Sol, Claude Fable 5.2, Claude Opus 5.5 and GPT-6.1 Sol on VulcanBench Frontier v4.

The compact 16:9 card (owner's minimal style, as for Beam and GPT-6.1 Terra):
the model and version as the title, then two charts, combined score and cost
per task at every effort level, and one footnote line that keeps the judged
run counts, the judge panels and the cost basis visible. The per-level table,
minutes included, is written to the CSV beside the card.

Comparators: Claude Fable 5.2 (first on the board), Claude Opus 5.5 and GPT-6.2
Sol's predecessor GPT-6.1 Sol, all priced at all five levels. Grok 4.7 (third
on the board) is left off because it has no Max level and no list price.

Sources, nothing re-run or re-judged:

- GPT-6.2 Sol: the Code quality v3.31 summary (Muse Spark 1.3 and Grok 4.6) and
  its run summaries in runs-effort-gpt62-sol. Cost is the API-equivalent at
  OpenAI's list price from the tokens Codex reported (subscription runs).
- The comparators: their published rows in the leaderboard table
  (VulcanBenchCOM assets/data/swe-v4-board.csv), whose sha256 is recorded in
  the card's JSON. Fable 5.2 is judged by Muse Spark 1.3 and GPT-6.1 Sol
  (v3.30.2); Opus 5.5 and GPT-6.1 Sol by Muse Spark 1.3 and Grok 4.6.

    python scripts/cii-v4-board/make_gpt62sol_vs_frontier_card.py --judging <v3.31 run directory> \\
        --board <VulcanBenchCOM checkout>/assets/data/swe-v4-board.csv
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

OUTPUT = ROOT / "docs/results/swe-v4-gpt62-sol-2026-11"
RUNS = ROOT / "runs-effort-gpt62-sol"
SPEC = "codex:gpt-6.2-sol"
STEM = "gpt62sol-vs-frontier"
MODELS = ("gpt62sol", "fable52", "opus55", "gpt61sol")
FRONTIER = ("fable52", "opus55", "gpt61sol")
BOARD_NAMES = {"fable52": "Fable 5.2", "opus55": "Opus 5.5", "gpt61sol": "GPT-6.1 Sol"}
NAMES = {
    "gpt62sol": "GPT-6.2 Sol",
    "fable52": "Claude Fable 5.2",
    "opus55": "Claude Opus 5.5",
    "gpt61sol": "GPT-6.1 Sol",
}
# GPT-6.2 Sol takes OpenAI green and Fable 5.2 Anthropic clay (the lab colors); Opus 5.5 keeps
# its deep brown from the Terra card and GPT-6.1 Sol moves to slate so the two OpenAI models
# differ. Markers, direct labels and value labels carry identity as well as color.
COLORS = {"gpt62sol": "#10A37F", "fable52": "#D97757", "opus55": "#8C3A1F", "gpt61sol": "#5C6370"}
MARKERS = {"gpt62sol": "o", "fable52": "D", "opus55": "s", "gpt61sol": "^"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def load_groups(judging: Path, board: Path):
    """Per-level aggregates: the comparators from the published board, GPT-6.2 Sol from v3.31."""
    groups = {}
    with board.open() as fh:
        for r in csv.DictReader(fh):
            for m, name in BOARD_NAMES.items():
                if r["model"] != name:
                    continue
                require((m, r["effort"]) not in groups, f"duplicate row {name} {r['effort']}")
                require(r["mean_usd"] != "", f"{name} {r['effort']} has no price")
                groups[m, r["effort"]] = {
                    "n": int(r["passed_of"]),
                    "combined": {
                        "n": int(r["n"]),
                        "mean": float(r["combined_33"]),
                        "se": float(r["combined_33_se"]),
                    },
                    "code_quality": {"mean": float(r["code_quality"])},
                    "passed": int(r["passed"]),
                    "usd": {"mean": float(r["mean_usd"])},
                    "minutes": {"mean": float(r["mean_minutes"])},
                    "protocol": r["protocol"],
                }
    summary = json.loads((judging / "summary.json").read_text())
    require(
        summary["failed_panels"] == []
        and summary["passing_panels"] == ["muse", "grok"]
        and summary["ready_for_publication"],
        "a v3.31 panel failed or the round is not ready",
    )
    for e in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / e).glob("legacy-*/summary.json"))]
        require(len(runs) == 23, f"GPT-6.2 Sol {e} is not 23 runs")
        require(all(r["model"] == SPEC for r in runs), f"GPT-6.2 Sol {e} has another solver")
        g = summary["groups"][f"gpt62sol/{e}"]
        published = [
            r["published"]["composite_v3"]
            for r in summary["rows"]
            if r["model"] == "gpt62sol" and r["effort"] == e and r.get("published")
        ]
        require(len(published) == g["composite_v3"]["n"] == 23, f"GPT-6.2 Sol {e} is not 23 judged")
        groups["gpt62sol", e] = {
            "n": 23,
            "combined": g["composite_v3"],
            "code_quality": g["code_quality"],
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "usd": {
                "mean": statistics.mean(r["economics"]["api_equivalent_cost_usd"] for r in runs)
            },
            "minutes": {"mean": statistics.mean(r["duration_s"] for r in runs) / 60},
            "protocol": summary["protocol"],
        }
    for m in MODELS:
        for e in LEVELS:
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "board_csv_sha256": digest(board.read_bytes()),
        "v3.31_summary_sha256": digest((judging / "summary.json").read_bytes()),
    }
    return groups, sources


def main():  # noqa: PLR0915, one linear figure
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True)
    parser.add_argument("--board", type=Path, required=True)
    args = parser.parse_args()
    groups, sources = load_groups(args.judging, args.board)

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 9.0
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
    text(right, 0.65, "Frontier v4  ·  November 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "GPT-6.2 Sol", 30, True, heading=True)
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])

    x = left
    for m in MODELS:
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.4)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=9,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        text(x + 0.016, 2.4, NAMES[m], 13, True)
        x += {"gpt62sol": 0.15, "fable52": 0.18, "opus55": 0.18, "gpt61sol": 0}[m]

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

    text(left, 3.0, "Combined score", 20, True, heading=True)
    top_h, gap_h, bot_h = 2.1, 0.14, 1.66
    front = [groups[m, e]["combined"] for m in FRONTIER for e in LEVELS]
    sol = [groups["gpt62sol", e]["combined"] for e in LEVELS]
    up_lo = math.floor(min(v["mean"] - v["se"] for v in front))
    up_hi = math.ceil(max(v["mean"] + v["se"] for v in front)) + 1
    sol_hi = max(v["mean"] + v["se"] for v in sol)
    broken = sol_hi + 3 < up_lo
    if broken:
        # Broken axis, as on the Beam card: a zoomed frontier band over GPT-6.2 Sol's own band.
        upper = axis(0.085, 0.40, 3.7, top_h)
        lower = axis(0.085, 0.40, 3.7 + top_h + gap_h, bot_h)
        lo_lo = 5 * math.floor((min(v["mean"] - v["se"] for v in sol) - 3) / 5)
        lo_hi = 5 * math.ceil((sol_hi + 2) / 5)
        require(
            lo_hi < up_lo,
            "GPT-6.2 Sol and the frontier band overlap; the broken axis would hide data",
        )
        note = f"higher is better  ·  axis broken between {lo_hi} and {up_lo}  ·  whiskers ±1 se"
        upper.set_ylim(up_lo, up_hi)
        upper.set_yticks(range(up_lo + up_lo % 2, up_hi + 1, 2))
        lower.set_ylim(lo_lo, lo_hi)
        lower.set_yticks(range(lo_lo + (-lo_lo) % 10, lo_hi + 1, 10))
        upper.spines["bottom"].set_visible(False)
        upper.tick_params(axis="x", labelbottom=False)
        for a, y0 in ((upper, 0), (lower, 1)):
            a.set_xlim(-0.35, len(LEVELS) - 0.35)
            dy = 0.06 / (top_h if a is upper else bot_h)
            a.plot(
                [-0.012, 0.012],
                [y0 - dy, y0 + dy],
                transform=a.transAxes,
                color=INK,
                lw=1,
                clip_on=False,
            )
        home = {m: upper for m in FRONTIER} | {"gpt62sol": lower}
        spans = {upper: up_hi - up_lo, lower: lo_hi - lo_lo}
    else:
        # GPT-6.2 Sol overlaps the frontier band, so one shared scale; no break.
        both = front + sol
        lo = math.floor(min(v["mean"] - v["se"] for v in both)) - 1
        hi = math.ceil(max(v["mean"] + v["se"] for v in both)) + 1
        single = axis(0.085, 0.40, 3.7, top_h + gap_h + bot_h)
        single.set_xlim(-0.35, len(LEVELS) - 0.35)
        single.set_ylim(lo, hi)
        step = 2 if hi - lo <= 16 else 5
        single.set_yticks(range(lo + (-lo) % step, hi + 1, step))
        note = "higher is better  ·  whiskers ±1 se"
        home = dict.fromkeys(MODELS, single)
        spans = {single: hi - lo}
    text(left, 3.32, note, 12, color=MUTED)
    # End labels: at least ~6% of their band apart so close finishes stay legible, then the
    # stack is re-centred on the line ends so no label drifts far from its line.
    label_y = {}
    for ax_ in set(home.values()):
        members = [m for m in MODELS if home[m] is ax_]
        end = {m: groups[m, "max"]["combined"]["mean"] for m in members}
        gap = 0.06 * spans[ax_]
        placed = {}
        for m in sorted(members, key=end.get):
            placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
        shift = statistics.mean(placed.values()) - statistics.mean(end.values())
        label_y |= {m: y - shift for m, y in placed.items()}
    dodge = {"gpt62sol": -0.03, "fable52": -0.09, "opus55": 0.03, "gpt61sol": 0.09}
    for m in MODELS:
        ax = home[m]
        xs = [LEVELS.index(e) + dodge[m] for e in LEVELS]
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        ax.plot(xs, ys, color=COLORS[m], lw=2, zorder=2)
        ax.errorbar(
            xs,
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
            (xs[-1], ys[-1]),
            xytext=(4.18, label_y[m]),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
            annotation_clip=False,
        )

    text(0.56, 3.0, "Cost per task", 20, True, heading=True)
    text(0.56, 3.32, "US dollars at list price  ·  lower is cheaper", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, top_h + gap_h + bot_h)
    # Four-character dollar labels are wider than a 0.2 bar, so bars sit 0.22 apart and the
    # labels are a size smaller than on the Beam card; neighbours' labels then never touch.
    shifts = {"gpt62sol": -0.33, "fable52": -0.11, "opus55": 0.11, "gpt61sol": 0.33}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(len(LEVELS))],
            ys,
            width=0.21,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in enumerate(ys):
            # A neighbour to the left of near equal height would share the label row; lift this one.
            prev = MODELS[MODELS.index(m) - 1] if MODELS.index(m) else None
            close = (
                prev is not None and abs(groups[prev, LEVELS[i]]["usd"]["mean"] - y) < 0.04 * top
            )
            ax.annotate(
                f"{y:.2f}" if y < 10 else f"{y:.1f}",
                (i + shifts[m], y),
                xytext=(0, 12 if close else 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=6.6,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    short = {m: NAMES[m].replace("Claude ", "") for m in MODELS}
    partial = "; ".join(
        short[m]
        + " "
        + ", ".join(
            f"{e.replace('-', ' ').capitalize()} {groups[m, e]['combined']['n']}"
            for e in LEVELS
            if groups[m, e]["combined"]["n"] < 23
        )
        for m in MODELS
        if any(groups[m, e]["combined"]["n"] < 23 for e in LEVELS)
    )
    judged = f"23 per point except {partial}" if partial else "23 per point"
    text(
        left,
        8.45,
        f"Judged runs: {judged}. Code quality judges: Muse Spark 1.3 with Grok 4.6 "
        "(Fable 5.2: with GPT-6.1 Sol). Cost is API-equivalent at list price. Harnesses differ.",
        11,
        color=MUTED,
    )

    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / f"{STEM}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"{STEM}-efforts.csv"
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
                "protocol",
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
                    "" if g["code_quality"]["mean"] is None else f"{g['code_quality']['mean']:.4f}",
                    g["passed"],
                    "" if g["usd"]["mean"] is None else f"{g['usd']['mean']:.4f}",
                    f"{g['minutes']['mean']:.4f}",
                    g["protocol"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "final": True,
            "sources": sources,
            "comparators": BOARD_NAMES,
            "best": best,
            "broken_axis": broken,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

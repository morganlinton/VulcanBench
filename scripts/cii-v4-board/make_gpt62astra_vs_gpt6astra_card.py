"""GPT-6 Astra and GPT-6.2 Astra on VulcanBench Frontier v4, head to head.

The compact 16:9 card in the owner's minimal style (as for the GPT-6.1 Sol vs GPT-6.2 Sol card):
the two models as the title, then two charts, combined score and cost per task at every
effort level, and one footnote that keeps run counts, pass counts, the judge panels and
the cost basis visible. The per-level table, minutes included, is written to the CSV beside
the card.

Sources, nothing re-run or re-judged:

- GPT-6.2 Astra: the Code quality v3.33 summary (Muse Spark 1.3 and Grok 4.6) and its run
  summaries in runs-effort-gpt62-astra. Cost is the API-equivalent at OpenAI's list price from
  the tokens Codex reported (subscription runs).
- GPT-6 Astra: its published rows in the leaderboard table (VulcanBenchCOM
  assets/data/swe-v4-board.csv, v3.4, Muse Spark 1.3 and Grok 4.6), whose sha256 is recorded
  in the card's JSON. Both ran through Codex; each is priced at its own OpenAI list rates
  (GPT-6 Astra $10.00 / $1.00 / $50.00, GPT-6.2 Astra $8.00 / $0.40 / $40.00 per 1M tokens).

    python scripts/cii-v4-board/make_gpt62astra_vs_gpt6astra_card.py --judging <v3.33 run directory> \\
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

OUTPUT = ROOT / "docs/results/swe-v4-gpt62-astra-2026-11"
RUNS = ROOT / "runs-effort-gpt62-astra"
SPEC = "codex:gpt-6.2-astra"
STEM = "gpt62astra-vs-gpt6astra"
MODELS = ("gpt6astra", "gpt62astra")
BOARD_NAMES = {"gpt6astra": "GPT-6 Astra"}
NAMES = {"gpt6astra": "GPT-6 Astra", "gpt62astra": "GPT-6.2 Astra"}
# GPT-6.2 Astra keeps OpenAI green and GPT-6 Astra the slate GPT-6.1 Sol has on its card;
# markers and direct labels carry identity as well as color.
COLORS = {"gpt6astra": "#5C6370", "gpt62astra": "#10A37F"}
MARKERS = {"gpt6astra": "^", "gpt62astra": "o"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def load_groups(judging: Path, board: Path):
    """Per-level aggregates: GPT-6 Astra from the published board, GPT-6.2 Astra from v3.33."""
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
        "a v3.33 panel failed or the round is not ready",
    )
    for e in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / e).glob("legacy-*/summary.json"))]
        require(len(runs) == 23, f"GPT-6.2 Astra {e} is not 23 runs")
        require(all(r["model"] == SPEC for r in runs), f"GPT-6.2 Astra {e} has another solver")
        g = summary["groups"][f"gpt62astra/{e}"]
        require(g["composite_v3"]["n"] == 23, f"GPT-6.2 Astra {e} is not 23 judged")
        groups["gpt62astra", e] = {
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
        "v3.33_summary_sha256": digest((judging / "summary.json").read_bytes()),
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
    text(left, 1.72, "GPT-6 Astra vs GPT-6.2 Astra", 30, True, heading=True)

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
        x += 0.12

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

    plot_h = 3.9
    text(left, 3.0, "Combined score", 20, True, heading=True)
    text(left, 3.32, "higher is better  ·  whiskers ±1 se", 12, color=MUTED)
    vals = [groups[m, e]["combined"] for m in MODELS for e in LEVELS]
    lo = math.floor(min(v["mean"] - v["se"] for v in vals)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 1
    ax = axis(0.085, 0.40, 3.7, plot_h)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    ax.set_ylim(lo, hi)
    ax.set_yticks(range(lo + lo % 2, hi + 1, 2) if hi - lo <= 16 else range(lo, hi + 1, 5))
    end = {m: groups[m, "max"]["combined"]["mean"] for m in MODELS}
    gap = 0.07 * (hi - lo)
    placed = {}
    for m in sorted(MODELS, key=end.get):
        placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
    shift = statistics.mean(placed.values()) - statistics.mean(end.values())
    dodge = {"gpt6astra": -0.04, "gpt62astra": 0.04}
    for m in MODELS:
        xs = [LEVELS.index(e) + dodge[m] for e in LEVELS]
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        ax.plot(xs, ys, color=COLORS[m], lw=2.2, zorder=2)
        ax.errorbar(
            xs,
            ys,
            yerr=es,
            fmt=MARKERS[m],
            color=COLORS[m],
            markersize=9,
            markeredgecolor=INK,
            markeredgewidth=0.6,
            ecolor=INK,
            elinewidth=0.8,
            capsize=3,
            zorder=3,
        )
        ax.annotate(
            NAMES[m],
            (xs[-1], ys[-1]),
            xytext=(4.18, placed[m] - shift),
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
    ax = axis(0.585, 0.355, 3.7, plot_h)
    shifts = {"gpt6astra": -0.16, "gpt62astra": 0.16}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.18)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.2f}")
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(len(LEVELS))],
            ys,
            width=0.30,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in enumerate(ys):
            ax.annotate(
                f"{y:.2f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=9,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    judged = {groups[m, e]["combined"]["n"] for m in MODELS for e in LEVELS}
    require(judged == {23}, "a point has fewer than 23 judged runs; the footnote would be wrong")
    short = [
        f"{groups['gpt6astra', e]['passed']} at {e.replace('-', ' ').capitalize()}"
        for e in LEVELS
        if groups["gpt6astra", e]["passed"] < 23
    ]
    require(
        all(groups["gpt62astra", e]["passed"] == 23 for e in LEVELS),
        "GPT-6.2 Astra missed a task; the footnote would be wrong",
    )
    require(len(short) == 1, "GPT-6 Astra pass counts changed; reword the footnote")
    text(
        left,
        8.45,
        f"23 judged runs per point. GPT-6 Astra passed {short[0].replace(' at', ' of 23 tasks at')}; "
        "every other point 23. Judges: Muse Spark 1.3 and Grok 4.6 (v3.4, v3.33). "
        "Cost is API-equivalent at each model's list price.",
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
        for m in MODELS:
            for e in LEVELS:
                g = groups[m, e]
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
                        g["protocol"],
                    ]
                )
    save(
        out.with_suffix(".json"),
        {
            "final": True,
            "sources": sources,
            "best": best,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in MODELS:
        print(f"  {NAMES[m]:<12} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

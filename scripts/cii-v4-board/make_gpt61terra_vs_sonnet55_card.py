"""GPT-6.1 Terra and Claude Sonnet 5.5 on VulcanBench Frontier v4.

Owner request 2026-10-31. Same compact 16:9 layout as the Terra card in
make_gpt61terra_vs_frontier_card.py: model as the title, combined score and
cost per task at every effort level, one footnote line. The per-level table,
minutes included, is written to the CSV beside the card. Muse Wonder Full was
on the first draft and was dropped by the owner: it ran in Contributor mode,
which has no Max level.

Source, nothing re-run or re-judged: the published leaderboard table
(VulcanBenchCOM assets/data/swe-v4-board.csv), which carries each model's
per-level rows (Terra v3.29, Sonnet v3.23, both judged by Muse Spark 1.3 and
Grok 4.6). Its sha256 is recorded in the card's JSON.

    python scripts/cii-v4-board/make_gpt61terra_vs_sonnet55_card.py \
        --board <VulcanBenchCOM checkout>/assets/data/swe-v4-board.csv
"""

from __future__ import annotations

import argparse
import csv
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

OUTPUT = ROOT / "docs/results/swe-v4-gpt61-terra-2026-10"
MODELS = ("gpt61terra", "sonnet55")
BOARD_NAMES = {"gpt61terra": "GPT-6.1 Terra", "sonnet55": "Sonnet 5.5"}
NAMES = {"gpt61terra": "GPT-6.1 Terra", "sonnet55": "Claude Sonnet 5.5"}
# Lab colors (OpenAI green, Anthropic clay). On the card paper the pair sits in the
# color-vision floor band (protan dE 6.8), so markers, direct labels and value labels
# carry identity as well as color.
COLORS = {"gpt61terra": "#10A37F", "sonnet55": "#D97757"}
MARKERS = {"gpt61terra": "o", "sonnet55": "D"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
STEM = "terra-vs-sonnet55"


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def load(board: Path):
    groups = {}
    with board.open() as fh:
        for r in csv.DictReader(fh):
            for m, name in BOARD_NAMES.items():
                if r["model"] == name:
                    require((m, r["effort"]) not in groups, f"duplicate row {name} {r['effort']}")
                    groups[m, r["effort"]] = {
                        "judged": int(r["n"]),
                        "of": int(r["passed_of"]),
                        "mean": float(r["combined_33"]),
                        "se": float(r["combined_33_se"]),
                        "code_quality": float(r["code_quality"]),
                        "passed": int(r["passed"]),
                        "usd": float(r["mean_usd"]),
                        "minutes": float(r["mean_minutes"]),
                        "protocol": r["protocol"],
                        "harness": r["harness"],
                    }
    require(set(groups) == {(m, e) for m in MODELS for e in LEVELS}, "a level is missing")
    # The footnote states one judged count for every point, so every point must be complete.
    require(len({(g["judged"], g["of"]) for g in groups.values()}) == 1, "judged counts differ")
    require(all(g["judged"] == g["of"] for g in groups.values()), "a point is partial")
    return groups


def main():  # noqa: PLR0915, one linear figure
    p = argparse.ArgumentParser()
    p.add_argument("--board", type=Path, required=True)
    p.add_argument("--out", type=Path, default=OUTPUT)
    args = p.parse_args()
    groups = load(args.board)

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 9.0
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, ha="left", color=INK):
        fam = ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium") if heading else "Geist"
        wt = (600 if bold else 500) if heading else (700 if bold else 400)
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
    text(right, 0.65, "Frontier v4  ·  October 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    text(left, 1.72, "GPT-6.1 Terra", 30, True, heading=True)
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
        x += 0.15

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

    # Combined score: the two curves overlap, so one shared scale.
    text(left, 3.0, "Combined score", 20, True, heading=True)
    text(left, 3.32, "higher is better  ·  whiskers ±1 se", 12, color=MUTED)
    span_h = 3.9
    pts = list(groups.values())
    lo = math.floor(min(g["mean"] - g["se"] for g in pts)) - 1
    hi = math.ceil(max(g["mean"] + g["se"] for g in pts)) + 1
    ax = axis(0.085, 0.40, 3.7, span_h)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    ax.set_ylim(lo, hi)
    step = 2 if hi - lo <= 16 else 5
    ax.set_yticks(range(lo + (-lo) % step, hi + 1, step))
    # End labels past the Max column, spaced apart when the two Max points sit close.
    end = {m: groups[m, "max"]["mean"] for m in MODELS}
    gap = 0.06 * (hi - lo)
    placed = {}
    for m in sorted(MODELS, key=end.get):
        placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
    shift = statistics.mean(placed.values()) - statistics.mean(end.values())
    dodge = {"gpt61terra": -0.04, "sonnet55": 0.04}
    for m in MODELS:
        xs = [i + dodge[m] for i in range(len(LEVELS))]
        ys = [groups[m, e]["mean"] for e in LEVELS]
        es = [groups[m, e]["se"] for e in LEVELS]
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
            xytext=(4.18, placed[m] - shift),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
            annotation_clip=False,
        )

    # Cost per task.
    text(0.56, 3.0, "Cost per task", 20, True, heading=True)
    text(0.56, 3.32, "US dollars at list price  ·  lower is cheaper", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, span_h)
    shifts = {"gpt61terra": -0.17, "sonnet55": 0.17}
    ax.set_ylim(0, max(g["usd"] for g in pts) * 1.18)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        xs = [i + shifts[m] for i in range(len(LEVELS))]
        ys = [groups[m, e]["usd"] for e in LEVELS]
        ax.bar(xs, ys, width=0.3, color=COLORS[m], edgecolor=INK, linewidth=0.5, zorder=2)
        for x_, y in zip(xs, ys, strict=True):
            ax.annotate(
                f"{y:.2f}",
                (x_, y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    judged = pts[0]["judged"]
    footnote = (
        f"{judged} judged runs per point. Code quality judges for both: Muse Spark 1.3 and "
        "Grok 4.6. Harnesses: Codex for Terra, Claude Code for Sonnet."
    )
    text(left, 8.45, footnote, 11, color=MUTED)

    args.out.mkdir(parents=True, exist_ok=True)
    out = args.out / f"{STEM}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = args.out / f"{STEM}-efforts.csv"
    cols = [
        "model",
        "effort",
        "harness",
        "protocol",
        "judged",
        "of",
        "combined",
        "combined_se",
        "code_quality",
        "passed",
        "usd_per_task",
        "minutes",
    ]
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(cols)
        for m in MODELS:
            for e in LEVELS:
                g = groups[m, e]
                w.writerow(
                    [
                        m,
                        e,
                        g["harness"],
                        g["protocol"],
                        g["judged"],
                        g["of"],
                        f"{g['mean']:.4f}",
                        f"{g['se']:.4f}",
                        f"{g['code_quality']:.4f}",
                        g["passed"],
                        f"{g['usd']:.4f}",
                        f"{g['minutes']:.4f}",
                    ]
                )
    save(
        args.out / f"{STEM}.json",
        {
            "source": "VulcanBenchCOM assets/data/swe-v4-board.csv",
            "source_sha256": digest(args.board.read_bytes()),
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
            "footnote": footnote,
        },
    )
    print(out)
    print(footnote)


if __name__ == "__main__":
    main()

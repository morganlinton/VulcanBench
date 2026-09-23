#!/usr/bin/env python3
"""Render the VulcanBench Verdict v1 model card for Jev.

Same house layout as the Frontier v4 model cards (masthead, title, legend,
two charts, one table, notes; see scripts/cii-v4-board/make_sol_v37_card.py):
accuracy against always guessing for every question, the distribution of
Jev's stated confidence that a fix works, and a table with ranking quality
and calibration. How the benchmark works is explained on the report page.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyBboxPatch, Patch

REPO = Path(__file__).resolve().parents[2]
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
# TypeSafe has no colour in the lab palette; deep magenta clears every existing
# lab hue, and like the rest every mark stays directly labelled.
JEV = "#B8407A"
GUESS = "#d9d7cf"
FAILS = "#8f8e86"
WIDTH_IN, HEIGHT_IN = 16, 11.5
LEFT, RIGHT = 0.06, 0.94
QUESTIONS = (
    ("patch-verdict", "Passes every\ntest?", "Does the fix pass every test?"),
    ("patch-regression", "Breaks an\nexisting test?", "Does it break an existing test?"),
    ("patch-outcome", "Which of four\noutcomes?", "Which of four test outcomes?"),
    ("quality-preference", "Which fix is\nbetter written?*", "Which fix is better written?*"),
)


def accuracy(metrics: dict) -> float:
    """Share correct, at the fitted cutoff where the question has one."""
    return metrics.get("accuracy_at_threshold", metrics["accuracy"])


def stderr(p: float, n: int) -> float:
    return math.sqrt(p * (1 - p) / n)


class Card:
    def __init__(self) -> None:
        self.fig = plt.figure(figsize=(WIDTH_IN, HEIGHT_IN), dpi=150, facecolor=PAPER)

    @staticmethod
    def yf(inches: float) -> float:
        """Figure fraction for a position measured in inches from the top edge."""
        return 1 - inches / HEIGHT_IN

    def text(
        self,
        x,
        y_in,
        label,
        size=16,
        bold=False,
        heading=False,
        numeric=False,
        ha="left",
        color=INK,
    ):
        if numeric:
            family, weight = "IBM Plex Mono", 500 if bold else 400
        elif heading:
            family, weight = (
                ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium"),
                (600 if bold else 500),
            )
        else:
            family, weight = "Geist", 700 if bold else 400
        self.fig.text(
            x,
            self.yf(y_in),
            label,
            fontsize=size,
            fontfamily=family,
            weight=weight,
            ha=ha,
            va="center",
            color=color,
        )

    def line(self, x1, x2, y_in, color=RULE, width=0.8):
        self.fig.add_artist(
            plt.Line2D(
                [x1, x2],
                [self.yf(y_in), self.yf(y_in)],
                transform=self.fig.transFigure,
                color=color,
                lw=width,
            )
        )

    def axes(self, x0, w, top_in, h_in):
        ax = self.fig.add_axes([x0, self.yf(top_in + h_in), w, h_in / HEIGHT_IN], facecolor=PAPER)
        ax.tick_params(axis="x", length=0, labelsize=11.5, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        return ax


def masthead(card: Card) -> None:
    logo = card.fig.add_axes([LEFT, card.yf(0.86), 0.42 / WIDTH_IN, 0.42 / HEIGHT_IN])
    mark = logo.imshow(plt.imread(REPO / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    card.text(LEFT + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    card.text(RIGHT, 0.65, "September 2026", 14, ha="right", color=MUTED)
    card.line(LEFT, RIGHT, 1.05, INK, 1.2)


def title(card: Card, data: dict) -> None:
    tests = sum(counts["test"] for counts in data["item_counts"].values())
    card.text(LEFT, 1.78, "VulcanBench Verdict v1: Jev 1.13.0", 33, True, heading=True)
    card.text(
        LEFT,
        2.22,
        f"{tests:,} questions about {data['source_patches']} code fixes written by AI agents. "
        "Every correctness answer is checked against the fix's real test results.",
        15,
        color=MUTED,
    )
    card.fig.add_artist(
        plt.Line2D(
            [LEFT + 0.004],
            [card.yf(2.68)],
            transform=card.fig.transFigure,
            marker="o",
            color=JEV,
            markersize=8,
            markeredgecolor=INK,
            markeredgewidth=0.6,
            linestyle="none",
        )
    )
    card.text(LEFT + 0.018, 2.68, "Jev 1.13.0  ·  TypeSafe API", 15, True)
    card.text(
        RIGHT,
        2.68,
        "n=611 per correctness question, n=311 style pairs, n=1,852 overall",
        13,
        ha="right",
        color=MUTED,
    )


def accuracy_chart(card: Card, data: dict) -> None:
    jev = data["results"][data["model"]]["families"]
    guess = data["results"]["majority_floor"]["families"]
    card.text(LEFT, 3.1, "Accuracy against always guessing", 21, True, heading=True)
    card.text(
        LEFT,
        3.42,
        "Percent correct  ·  higher is better  ·  whiskers ±1 standard error",
        12,
        color=MUTED,
    )
    ax = card.axes(0.085, 0.405, 3.75, 3.0)
    width = 0.36
    for i, (family, _label, _) in enumerate(QUESTIONS):
        p_jev, p_guess = accuracy(jev[family]), accuracy(guess[family])
        n = jev[family]["n"]
        ax.bar(
            i - width / 2, p_guess * 100, width, color=GUESS, edgecolor=INK, linewidth=0.5, zorder=2
        )
        ax.bar(
            i + width / 2,
            p_jev * 100,
            width,
            color=JEV,
            edgecolor=INK,
            linewidth=0.5,
            yerr=stderr(p_jev, n) * 100,
            error_kw={"ecolor": INK, "elinewidth": 0.9, "capsize": 3},
            zorder=2,
        )
        for x, value, offset in (
            (i - width / 2, p_guess, 0),
            (i + width / 2, p_jev, stderr(p_jev, n)),
        ):
            ax.annotate(
                f"{value * 100:.0f}",
                (x, (value + offset) * 100),
                xytext=(0, 5),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=10.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )
    ax.set_xticks(range(len(QUESTIONS)), [label for _, label, _ in QUESTIONS])
    ax.set_xlim(-0.6, len(QUESTIONS) - 0.4)
    ax.set_ylim(0, 112)
    ax.set_yticks(range(0, 101, 25))
    ax.legend(
        handles=[
            Patch(facecolor=GUESS, edgecolor=INK, linewidth=0.5, label="Always guessing"),
            Patch(facecolor=JEV, edgecolor=INK, linewidth=0.5, label="Jev"),
        ],
        loc="upper left",
        frameon=False,
        fontsize=11,
        ncols=2,
        bbox_to_anchor=(0, 1.08),
    )


def confidence_chart(card: Card, data: dict) -> None:
    histogram = data["diagnostics"]["patch_verdict_p_true_histogram"]
    top = data["results"][data["model"]]["families"]["patch-verdict"]["p_true_max"]
    x0 = 0.565
    card.text(x0 - 0.04, 3.1, "Jev's confidence that a fix works", 21, True, heading=True)
    card.text(
        x0 - 0.04,
        3.42,
        "611 fixes, split by what the tests showed  ·  a yes needs 50%",
        12,
        color=MUTED,
    )
    ax = card.axes(x0, 0.375, 3.75, 3.0)
    step = histogram["step"] * 100
    edges = [float(edge) * 100 for edge in histogram["bins"]]
    works = [counts["passes"] for counts in histogram["bins"].values()]
    fails = [counts["fails"] for counts in histogram["bins"].values()]
    ax.bar(
        edges,
        works,
        width=step * 0.9,
        align="edge",
        color=JEV,
        edgecolor=INK,
        linewidth=0.5,
        zorder=2,
        label="Fix really works",
    )
    ax.bar(
        edges,
        fails,
        width=step * 0.9,
        align="edge",
        bottom=works,
        color=FAILS,
        edgecolor=INK,
        linewidth=0.5,
        zorder=2,
        label="Fix really fails",
    )
    ax.axvline(50, color=INK, linewidth=1.1, linestyle=(0, (4, 3)), zorder=3)
    peak = max(w + f for w, f in zip(works, fails, strict=True))
    ax.set_ylim(0, peak * 1.18)
    ax.set_xlim(0, 62)
    ax.set_xticks(range(0, 61, 10), [f"{v}%" for v in range(0, 61, 10)])
    last_height = works[-1] + fails[-1]
    ax.annotate(
        f"Highest: {top * 100:.0f}%",
        (edges[-1] + step * 0.45, last_height),
        xytext=(36, peak * 0.42),
        ha="left",
        fontsize=10.5,
        fontfamily="IBM Plex Mono",
        color=INK,
        arrowprops={"arrowstyle": "-", "color": INK, "lw": 0.8},
    )
    ax.text(49, peak * 1.08, "Yes threshold", ha="right", va="center", fontsize=10.5, color=INK)
    ax.legend(loc="upper left", frameon=False, fontsize=11)


def table(card: Card, data: dict) -> float:
    jev = data["results"][data["model"]]["families"]
    guess = data["results"]["majority_floor"]["families"]
    overall_jev = data["results"][data["model"]]["overall"]
    overall_guess = data["results"]["majority_floor"]["overall"]
    card.text(LEFT, 7.55, "Table 1  |  Every question", 14.5, heading=True)
    card.line(LEFT, RIGHT, 7.83, INK, 1.2)
    heads = ("Items", "Always guessing", "Jev", "Ranking (AUROC)", "Calibration error")
    cols = (0.46, 0.585, 0.695, 0.82, 0.94)
    card.text(LEFT, 8.07, "Question", 12.5, True)
    for head, x in zip(heads, cols, strict=True):
        card.text(x, 8.07, head, 12.5, True, ha="right")
    card.text(
        RIGHT,
        8.33,
        "Accuracy in %  ·  AUROC 0.5 is chance, 1 is perfect  ·  calibration error 0 is perfect",
        10,
        ha="right",
        color=MUTED,
    )
    card.line(LEFT, RIGHT, 8.5, INK, 0.6)

    rows = [
        (label, jev[family], guess[family], False)
        for family, _, label in QUESTIONS
        if family != "quality-preference"
    ]
    rows.append(("Overall, test-checked questions", overall_jev, overall_guess, True))
    rows.append((QUESTIONS[-1][2], jev["quality-preference"], guess["quality-preference"], False))
    y, step = 8.74, 0.35
    for label, mine, base, emphasis in rows:
        values = (
            f"{mine['n']:,}",
            f"{accuracy(base) * 100:.1f}",
            f"{accuracy(mine) * 100:.1f}",
            f"{mine['auroc']:.2f}" if mine.get("auroc") is not None else "",
            f"{mine['ece']:.2f}",
        )
        card.text(LEFT, y, label, 13 if emphasis else 12.5, emphasis)
        for value, x in zip(values, cols, strict=True):
            card.text(x, y, value, 14 if emphasis else 13.5, emphasis, numeric=True, ha="right")
        if emphasis:
            card.line(LEFT, RIGHT, y + step / 2, RULE, 0.6)
        y += step
    card.line(LEFT, RIGHT, y - step / 2, INK, 1.2)
    return y - step / 2


def notes(card: Card, data: dict, top: float) -> None:
    cutoffs = data["thresholds"]["values"]
    run = data["run"]
    lines = (
        "Jev's yes or no answers use a cutoff tuned on a separate set of fixes "
        f"({cutoffs['patch-verdict']:.2f} and {cutoffs['patch-regression']:.2f}); at the usual 50% it never answers yes. "
        "Always guessing gives each question's most common answer.",
        "*Checked against the Muse Spark 1.3 and Grok 4.6 code-quality panel, which is judgment rather than test results, "
        "so it is left out of the overall row.",
        f"{run['items_queried']:,} queries, no failures, ${run['total_cost_usd']:.2f} in total, "
        f"{run['latency_ms_p50']:.0f} ms median latency measured from California. Method and every number: "
        "vulcanbench.com/benchmarks/verdict-v1-jev.html",
    )
    for i, note in enumerate(lines):
        card.text(LEFT, top + 0.2 + 0.24 * i, note, 11, color=MUTED)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    results_dir = REPO / "docs" / "results" / "verdict-v1-jev-2026-09"
    parser.add_argument("--results", type=Path, default=results_dir / "verdict-v1-jev.json")
    parser.add_argument("-o", "--out", type=Path, default=results_dir / "verdict-v1-jev.png")
    args = parser.parse_args()
    for font in (REPO / "scripts" / "rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    data = json.loads(args.results.read_text())

    card = Card()
    masthead(card)
    title(card, data)
    accuracy_chart(card, data)
    confidence_chart(card, data)
    notes(card, data, table(card, data))
    card.fig.savefig(args.out, facecolor=PAPER)
    plt.close(card.fig)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

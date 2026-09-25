#!/usr/bin/env python3
"""Render a 16:9 "how VulcanBench Verdict works" graphic for social posts.

A method explainer, deliberately free of results: the four-step flow, the
four question types and the size of the suite. Results live on the model
card and the report page. House styling as the model cards (paper, Chakra
Petch headings, IBM Plex Mono numbers, Geist body; see make_jev_card.py).
"""

from __future__ import annotations

import argparse
import json
import textwrap
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch

REPO = Path(__file__).resolve().parents[2]
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
TILE = "#efece4"
WIDTH_IN, HEIGHT_IN = 16, 9
LEFT, RIGHT = 0.05, 0.95
GAP = 0.028


def yf(inches: float) -> float:
    return 1 - inches / HEIGHT_IN


def text(
    fig,
    x,
    y_in,
    label,
    size,
    *,
    face="body",
    bold=False,
    ha="left",
    va="center",
    color=INK,
    wrap=None,
):
    family, weight = {
        "body": ("Geist", 700 if bold else 400),
        "head": ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium", 600 if bold else 500),
        "mono": ("IBM Plex Mono", 500 if bold else 400),
    }[face]
    if wrap:
        label = "\n".join(textwrap.wrap(label, wrap))
    fig.text(
        x,
        yf(y_in),
        label,
        fontsize=size,
        fontfamily=family,
        weight=weight,
        ha=ha,
        va=va,
        color=color,
        linespacing=1.3,
    )


def tile(fig, x, top_in, w, h_in, face=TILE, edge=None):
    fig.patches.append(
        FancyBboxPatch(
            (x, yf(top_in + h_in)),
            w,
            h_in / HEIGHT_IN,
            boxstyle="round,pad=0,rounding_size=0.008",
            transform=fig.transFigure,
            facecolor=face,
            edgecolor=edge or face,
            linewidth=1.0,
        )
    )


def columns(n: int) -> list[tuple[float, float]]:
    width = (RIGHT - LEFT - GAP * (n - 1)) / n
    return [(LEFT + i * (width + GAP), width) for i in range(n)]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    results_dir = REPO / "docs" / "results" / "verdict-v1-jev-2026-09"
    parser.add_argument("--results", type=Path, default=results_dir / "verdict-v1-jev.json")
    parser.add_argument(
        "-o", "--out", type=Path, default=results_dir / "verdict-v1-how-it-works.png"
    )
    args = parser.parse_args()
    for font in (REPO / "scripts" / "rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK})
    data = json.loads(args.results.read_text())
    tests = sum(counts["test"] for counts in data["item_counts"].values())
    tasks = data["item_counts"]["fix-localization"]["total"]

    fig = plt.figure(figsize=(WIDTH_IN, HEIGHT_IN), dpi=150, facecolor=PAPER)

    # Masthead.
    logo = fig.add_axes([LEFT, yf(0.84), 0.42 / WIDTH_IN, 0.42 / HEIGHT_IN])
    mark = logo.imshow(plt.imread(REPO / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(fig, LEFT + 0.036, 0.63, "VulcanBench", 20, face="head", bold=True)
    text(fig, RIGHT, 0.63, "Verdict v1", 16, face="head", ha="right", color=MUTED)
    fig.add_artist(
        plt.Line2D([LEFT, RIGHT], [yf(1.02)] * 2, transform=fig.transFigure, color=INK, lw=1.2)
    )

    # Title.
    text(fig, LEFT, 1.62, "How VulcanBench Verdict works", 34, face="head", bold=True)
    text(
        fig,
        LEFT,
        2.08,
        "A benchmark for AI models that judge code instead of writing it. "
        "Whether a fix works is decided by real tests, not by another model.",
        14.5,
        color=MUTED,
    )

    # Four-step flow.
    steps = (
        (
            "An AI agent fixes a bug",
            f"Frontier coding agents wrote {data['source_patches']} fixes across {tasks} real engineering tasks.",
        ),
        (
            "Hidden tests grade it",
            "Tests the agent never saw record whether the fix passes, partly passes or breaks something.",
        ),
        (
            "The model reviews it",
            "It sees the bug report and the fix, never the test results, and answers with a probability.",
        ),
        (
            "We score the answer",
            "Against what the tests actually showed, and against always giving the most common answer.",
        ),
    )
    top, height = 2.62, 2.35
    for i, ((x, w), (title, detail)) in enumerate(zip(columns(4), steps, strict=True)):
        tile(fig, x, top, w, height)
        cx, cy = x + 0.028, yf(top + 0.42)
        fig.add_artist(
            plt.Line2D(
                [cx],
                [cy],
                marker="o",
                markersize=30,
                markerfacecolor=PAPER,
                markeredgecolor=INK,
                markeredgewidth=1.4,
                transform=fig.transFigure,
            )
        )
        fig.text(
            cx,
            cy,
            str(i + 1),
            fontsize=15,
            fontfamily="IBM Plex Mono",
            weight=500,
            ha="center",
            va="center",
        )
        text(fig, x + 0.018, top + 0.8, title, 16, face="head", bold=True, va="top", wrap=26)
        text(fig, x + 0.018, top + 1.28, detail, 12.5, va="top", color="#3a3a36", wrap=34)
    for x, w in columns(4)[:-1]:
        fig.patches.append(
            FancyArrowPatch(
                (x + w + 0.003, yf(top + height / 2)),
                (x + w + GAP - 0.003, yf(top + height / 2)),
                transform=fig.transFigure,
                arrowstyle="-|>",
                mutation_scale=16,
                color=INK,
                linewidth=1.4,
            )
        )

    # The questions.
    text(fig, LEFT, 5.42, "What the model is asked", 17, face="head", bold=True)
    questions = (
        ("Does the fix pass every test?", "yes or no"),
        ("Does it break a test that used to pass?", "yes or no"),
        ("Which of four outcomes did the tests report?", "pick one of four"),
        ("Which of two fixes is better written?*", "pick one of two"),
    )
    q_top, q_h = 5.72, 1.3
    for (x, w), (question, kind) in zip(columns(4), questions, strict=True):
        tile(fig, x, q_top, w, q_h, face=PAPER, edge=RULE)
        text(fig, x + 0.016, q_top + 0.2, kind.upper(), 10, face="mono", va="top", color=MUTED)
        text(fig, x + 0.016, q_top + 0.48, question, 13.5, face="head", va="top", wrap=28)

    # Scale and footer.
    fig.add_artist(
        plt.Line2D([LEFT, RIGHT], [yf(7.28)] * 2, transform=fig.transFigure, color=RULE, lw=0.8)
    )
    stats = (
        (f"{data['source_patches']}", "fixes"),
        (f"{tasks}", "tasks"),
        (f"{tests:,}", "scored questions"),
    )
    x = LEFT
    for value, label in stats:
        text(fig, x, 7.72, value, 22, face="mono", bold=True)
        text(fig, x + 0.0118 * len(value) + 0.008, 7.74, label, 13.5, color=MUTED)
        x += 0.17
    text(fig, RIGHT, 7.72, "First model tested: TypeSafe Jev 1.13.0", 13.5, face="head", ha="right")
    text(
        fig,
        LEFT,
        8.3,
        "*The style question is checked against an AI code-review panel (Muse Spark 1.3 and Grok 4.6). "
        "Every other answer is checked against real tests.",
        11,
        color=MUTED,
    )
    text(
        fig,
        LEFT,
        8.62,
        "vulcanbench.com/benchmarks/verdict-v1-jev.html",
        12,
        face="head",
        color="#3a3a36",
    )

    fig.savefig(args.out, facecolor=PAPER)
    plt.close(fig)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

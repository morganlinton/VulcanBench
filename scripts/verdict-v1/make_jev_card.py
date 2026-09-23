#!/usr/bin/env python3
"""Render the shareable VulcanBench Verdict v1 card for Jev.

Written for someone who has never heard of the suite: the top half says how
the test works in three steps, the bottom half gives three results in plain
words, each against the score for simply guessing. The full metric set
(AUROC, calibration, the fitted cutoffs) lives on the report page, not here.
Monochrome brand styling (see CLAUDE.md); Jev is ink, guessing is muted.
"""

from __future__ import annotations

import argparse
import json
import sys
import textwrap
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch, Rectangle

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts" / "rankings-chart"))

from _common import BRAND, BRAND_MED, GRID, INK, INK2, MUTED, SURFACE, register_fonts  # noqa: E402
from matplotlib import font_manager  # noqa: E402

MONO = "IBM Plex Mono"  # brand's secondary face, used for every number on the card
GUESS = "#c9c8c1"
TILE = "#f3f2ee"
W, H, DPI = 2560, 1440, 200
LEFT, RIGHT = 0.055, 0.945
GAP = 0.03
STEP_TOP, STEP_H = 0.785, 0.205
TILE_TOP, TILE_H = 0.49, 0.34


def columns(n: int) -> list[tuple[float, float]]:
    width = (RIGHT - LEFT - GAP * (n - 1)) / n
    return [(LEFT + i * (width + GAP), width) for i in range(n)]


def rounded_box(fig, x: float, y: float, w: float, h: float) -> None:
    fig.patches.append(
        FancyBboxPatch(
            (x, y),
            w,
            h,
            transform=fig.transFigure,
            boxstyle="round,pad=0,rounding_size=0.012",
            linewidth=0,
            facecolor=TILE,
        )
    )


def wrapped(fig, x: float, y: float, text: str, width: int, **style) -> None:
    fig.text(x, y, "\n".join(textwrap.wrap(text, width)), va="top", linespacing=1.25, **style)


def heading(fig, x: float, y: float, text: str, size: float) -> None:
    fig.text(x, y, text, fontsize=size, color=INK, fontfamily=BRAND_MED, va="center")


def bar(fig, x: float, y: float, w: float, color: str) -> None:
    fig.patches.append(
        Rectangle((x, y - 0.014), w, 0.028, transform=fig.transFigure, facecolor=color, linewidth=0)
    )


def comparison_bars(fig, x: float, y: float, w: float, jev: float, guess: float) -> None:
    """Two labelled horizontal bars, Jev over guessing, on a 0 to 100 scale."""
    track_x, track_w = x + 0.118, w - 0.2
    for row, (label, value, color) in enumerate(
        (("Jev", jev, INK), ("Always guessing", guess, GUESS))
    ):
        top = y - row * 0.062
        fig.text(x, top, label, fontsize=12, color=INK2, va="center")
        bar(fig, track_x, top, track_w, GRID)
        bar(fig, track_x, top, track_w * value / 100, color)
        fig.text(
            track_x + track_w + 0.008,
            top,
            f"{value:.0f}%",
            fontsize=15,
            color=INK,
            fontfamily=MONO,
            fontweight=500,
            va="center",
        )


def accuracy(source: dict, family: str) -> float:
    """Percent correct, at the fitted cutoff where the family has one."""
    metrics = source[family]
    return metrics.get("accuracy_at_threshold", metrics["accuracy"]) * 100


def draw_header(fig) -> None:
    logo = plt.imread(str(REPO / "scripts" / "rankings-chart" / "vb_logo_rounded.png"))
    logo_ax = fig.add_axes([LEFT, 0.885, 0.048, 0.085])
    logo_ax.imshow(logo)
    logo_ax.axis("off")
    fig.text(0.108, 0.938, "VulcanBench", fontsize=27, color=INK, fontfamily=BRAND, va="center")
    fig.text(0.108, 0.900, "Verdict v1", fontsize=17, color=INK2, fontfamily=BRAND_MED, va="center")
    fig.text(
        RIGHT,
        0.938,
        "Jev 1.13.0",
        fontsize=24,
        color=INK,
        fontfamily=BRAND_MED,
        ha="right",
        va="center",
    )
    fig.text(RIGHT, 0.900, "TypeSafe AI", fontsize=15, color=INK2, ha="right", va="center")


def draw_steps(fig, data: dict) -> None:
    heading(fig, LEFT, 0.825, "How the test works", 18)
    steps = (
        (
            "1",
            "An AI agent fixes a bug",
            f"We use {data['source_patches']} real fixes written during our coding benchmark.",
        ),
        (
            "2",
            "Jev reviews the fix",
            "It reads the bug report and the fix, then answers one question about it.",
        ),
        (
            "3",
            "We check the answer",
            "Every fix was already run against real tests, so we know the right answer.",
        ),
    )
    for (x, w), (number, title, detail) in zip(columns(3), steps, strict=True):
        rounded_box(fig, x, STEP_TOP - STEP_H, w, STEP_H)
        fig.text(
            x + 0.018,
            STEP_TOP - 0.038,
            number,
            fontsize=22,
            color=INK,
            fontfamily=MONO,
            fontweight=500,
            va="center",
        )
        heading(fig, x + 0.05, STEP_TOP - 0.038, title, 15)
        wrapped(fig, x + 0.05, STEP_TOP - 0.078, detail, 33, fontsize=12.5, color=INK2)
    for x, w in columns(3)[:-1]:
        fig.patches.append(
            FancyArrowPatch(
                (x + w + 0.004, STEP_TOP - STEP_H / 2),
                (x + w + GAP - 0.004, STEP_TOP - STEP_H / 2),
                transform=fig.transFigure,
                arrowstyle="-|>",
                mutation_scale=14,
                color=INK2,
                linewidth=1.4,
            )
        )


def draw_results(fig, data: dict) -> None:
    jev = data["results"][data["model"]]["families"]
    guess = data["results"]["majority_floor"]["families"]
    verdict = data["diagnostics"]["confusion"]["patch-verdict"]
    really_pass = verdict.get("true->true", 0) + verdict.get("true->false", 0)
    said_pass = verdict.get("true->true", 0) + verdict.get("false->true", 0)

    heading(fig, LEFT, 0.535, "What we found", 18)
    (x1, w1), (x2, w2), (x3, w3) = columns(3)
    for x, w in ((x1, w1), (x2, w2), (x3, w3)):
        rounded_box(fig, x, TILE_TOP - TILE_H, w, TILE_H)
    caption_y = TILE_TOP - 0.198
    caption = {"fontsize": 12.5, "color": INK2}

    heading(fig, x1 + 0.02, TILE_TOP - 0.04, "Does this fix work?", 16)
    comparison_bars(
        fig,
        x1 + 0.02,
        TILE_TOP - 0.1,
        w1 - 0.02,
        accuracy(jev, "patch-verdict"),
        accuracy(guess, "patch-verdict"),
    )
    wrapped(
        fig,
        x1 + 0.02,
        caption_y,
        "No better than always answering yes. On two related questions it did worse than guessing.",
        38,
        **caption,
    )

    heading(fig, x2 + 0.02, TILE_TOP - 0.04, "Does it ever say a fix works?", 16)
    fig.text(
        x2 + 0.02,
        TILE_TOP - 0.115,
        f"{said_pass}",
        fontsize=46,
        color=INK,
        fontfamily=MONO,
        fontweight=500,
        va="center",
    )
    fig.text(
        x2 + 0.075,
        TILE_TOP - 0.1,
        f"times out of {sum(verdict.values())}",
        fontsize=13,
        color=INK2,
        va="center",
    )
    fig.text(
        x2 + 0.075,
        TILE_TOP - 0.13,
        f"{really_pass} of them do work",
        fontsize=13,
        color=INK2,
        va="center",
    )
    wrapped(
        fig,
        x2 + 0.02,
        caption_y,
        "It never rated a fix as more likely to work than not. Its highest "
        f"confidence in any fix was {jev['patch-verdict']['p_true_max'] * 100:.0f}%.",
        38,
        **caption,
    )

    heading(fig, x3 + 0.02, TILE_TOP - 0.04, "Which fix is better written?*", 16)
    comparison_bars(
        fig,
        x3 + 0.02,
        TILE_TOP - 0.1,
        w3 - 0.02,
        accuracy(jev, "quality-preference"),
        accuracy(guess, "quality-preference"),
    )
    wrapped(
        fig,
        x3 + 0.02,
        caption_y,
        "Its strongest result: it picked the fix our AI code reviewers preferred "
        "9\u00a0times\u00a0in\u00a010.",
        38,
        **caption,
    )


def draw_footer(fig, data: dict) -> None:
    tests = sum(counts["test"] for counts in data["item_counts"].values())
    notes = (
        f'{tests:,} questions about {data["source_patches"]} fixes. "Always guessing" means giving the most '
        "common answer every time.",
        "Jev's yes or no answers use a cutoff tuned on a separate set of fixes, so the comparison is fair to it.",
        "*Checked against two AI reviewers (Muse Spark 1.3 and Grok 4.6), which is judgment. "
        "Every other answer is checked against real tests.",
    )
    for y, note in zip((0.118, 0.090, 0.062), notes, strict=True):
        fig.text(LEFT, y, note, fontsize=11.5, color=MUTED, va="center")
    fig.text(
        LEFT,
        0.026,
        "Full method and every number: vulcanbench.com/benchmarks/verdict-v1-jev.html",
        fontsize=12,
        color=INK2,
        fontfamily=BRAND_MED,
        va="center",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--results",
        type=Path,
        default=REPO / "docs" / "results" / "verdict-v1-jev-2026-09" / "verdict-v1-jev.json",
    )
    parser.add_argument(
        "-o",
        "--out",
        type=Path,
        default=REPO / "docs" / "results" / "verdict-v1-jev-2026-09" / "verdict-v1-jev.png",
    )
    args = parser.parse_args()
    register_fonts()
    for weight in (400, 500):
        font_manager.fontManager.addfont(
            str(REPO / "scripts" / "rankings-chart" / f"ibm-plex-mono-{weight}.ttf")
        )
    data = json.loads(args.results.read_text())

    fig = plt.figure(figsize=(W / DPI, H / DPI), dpi=DPI, facecolor=SURFACE)
    draw_header(fig)
    draw_steps(fig, data)
    draw_results(fig, data)
    draw_footer(fig, data)
    fig.patches.append(
        FancyBboxPatch(
            (0.004, 0.004),
            0.992,
            0.992,
            transform=fig.transFigure,
            boxstyle="round,pad=0,rounding_size=0.012",
            linewidth=1.2,
            edgecolor=GRID,
            facecolor="none",
        )
    )
    fig.savefig(args.out, facecolor=SURFACE)
    print(f"wrote {args.out} ({W}x{H})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

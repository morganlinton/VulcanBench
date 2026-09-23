#!/usr/bin/env python3
"""Render the shareable VulcanBench Verdict v1 card for Jev.

Three panels: accuracy against the majority-answer floor, what Jev actually
predicted on the 611 patch-verdict items, and accuracy against patch length.
Monochrome brand styling (see CLAUDE.md); Jev is ink, the floor is muted.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts" / "rankings-chart"))

from _common import BRAND, BRAND_MED, GRID, INK, INK2, MUTED, SURFACE, register_fonts  # noqa: E402
from matplotlib import font_manager  # noqa: E402

MONO = "IBM Plex Mono"  # brand's secondary face, used for every number on the card
JEV = INK
FLOOR = "#c9c8c1"
W, H, DPI = 2560, 1440, 200
FAMILIES = [
    ("patch-verdict", "Does this patch\npass every test?"),
    ("patch-regression", "Does it break\nan existing test?"),
    ("patch-outcome", "Which of four\noutcomes?"),
    ("quality-preference", "Which patch is more\nmaintainable?*"),
]


def panel_accuracy(ax, data: dict) -> None:
    jev = data["results"][data["model"]]["families"]
    floor = data["results"]["majority_floor"]["families"]
    x = range(len(FAMILIES))
    width = 0.38
    for offset, source, color, label in (
        (-width / 2, floor, FLOOR, "Always answer the most common label"),
        (width / 2, jev, JEV, "Jev"),
    ):
        values = [source[f]["accuracy"] * 100 for f, _ in FAMILIES]
        bars = ax.bar([i + offset for i in x], values, width, color=color, label=label, zorder=3)
        for bar, value in zip(bars, values, strict=True):
            ax.text(
                bar.get_x() + bar.get_width() / 2,
                value + 1.6,
                f"{value:.0f}",
                ha="center",
                va="bottom",
                fontsize=13,
                color=INK if color == JEV else INK2,
                fontfamily=MONO,
                fontweight=500,
            )
    ax.set_xticks(list(x))
    ax.set_xticklabels([label for _, label in FAMILIES], fontsize=12, color=INK2)
    ax.set_ylim(0, 108)
    ax.set_ylabel("accuracy, percent", fontsize=12, color=INK2)
    ax.set_title(
        "Jev is worse than guessing on correctness, strong on style",
        fontsize=16,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
    )
    ax.legend(frameon=False, fontsize=12, loc="lower right", bbox_to_anchor=(1, 1.0), ncols=2)


def panel_prediction(ax, data: dict) -> None:
    confusion = data["diagnostics"]["confusion"]["patch-verdict"]
    truth_pass = confusion.get("true->true", 0) + confusion.get("true->false", 0)
    said_pass = confusion.get("true->true", 0) + confusion.get("false->true", 0)
    total = sum(confusion.values())
    rows = [
        ("Patches that really pass", truth_pass, JEV),
        ("Patches Jev said would pass", said_pass, "#b34a3a"),
    ]
    for i, (label, value, color) in enumerate(rows):
        y = len(rows) - 1 - i
        ax.barh(y, value, height=0.36, color=color, zorder=3)
        ax.text(
            max(value, 0) + total * 0.015,
            y,
            f"{value} of {total}",
            va="center",
            fontsize=14,
            color=INK2,
            fontfamily=MONO,
        )
        ax.text(0, y + 0.3, label, va="bottom", fontsize=13, color=INK)
    ax.set_yticks([])
    ax.set_xlim(0, total * 1.2)
    ax.set_ylim(-0.5, 1.75)
    mean_p = data["diagnostics"]["patch_verdict_mean_p_pass"]
    ax.set_title(
        f"It called every patch broken (mean confidence: {mean_p:.2f})",
        fontsize=15,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
    )
    ax.set_xlabel("patches judged", fontsize=12, color=INK2)


def panel_length(ax, data: dict) -> None:
    buckets = data["diagnostics"]["patch_verdict_accuracy_by_input_tokens"]
    labels = list(buckets)
    values = [buckets[k]["accuracy"] * 100 for k in labels]
    ax.plot(
        range(len(labels)), values, color=JEV, linewidth=2.6, marker="o", markersize=9, zorder=3
    )
    for i, (label, value) in enumerate(zip(labels, values, strict=True)):
        ax.text(
            i,
            value + 3.4,
            f"{value:.0f}%",
            ha="center",
            fontsize=13,
            color=INK,
            fontfamily=MONO,
            fontweight=500,
        )
        ax.text(
            i,
            -7,
            f"n={buckets[label]['n']}",
            ha="center",
            fontsize=11,
            color=MUTED,
            fontfamily=MONO,
        )
    ax.set_xticks(range(len(labels)))
    ax.set_xticklabels(labels, fontsize=12, color=INK2)
    ax.set_xlim(-0.4, len(labels) - 0.6)
    ax.set_ylim(-12, 62)
    ax.set_xlabel("input size, tokens", fontsize=12, color=INK2)
    ax.set_ylabel("accuracy, percent", fontsize=12, color=INK2)
    ax.set_title(
        "The longer the patch, the worse it does",
        fontsize=16,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
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
    gs = fig.add_gridspec(
        2, 2, left=0.055, right=0.975, top=0.785, bottom=0.175, hspace=0.62, wspace=0.16
    )
    axes = [fig.add_subplot(gs[0, :]), fig.add_subplot(gs[1, 0]), fig.add_subplot(gs[1, 1])]
    for ax in axes:
        ax.set_facecolor(SURFACE)
        for side in ("top", "right"):
            ax.spines[side].set_visible(False)
        for side in ("left", "bottom"):
            ax.spines[side].set_color(GRID)
        ax.tick_params(colors=INK2, length=0)
        ax.grid(axis="y", color=GRID, linewidth=0.9, zorder=0)
    panel_accuracy(axes[0], data)
    panel_prediction(axes[1], data)
    panel_length(axes[2], data)

    logo = plt.imread(str(REPO / "scripts" / "rankings-chart" / "vb_logo_rounded.png"))
    logo_ax = fig.add_axes([0.055, 0.885, 0.048, 0.085])
    logo_ax.imshow(logo)
    logo_ax.axis("off")
    fig.text(0.108, 0.938, "VulcanBench", fontsize=27, color=INK, fontfamily=BRAND, va="center")
    fig.text(0.108, 0.900, "Verdict v1", fontsize=17, color=INK2, fontfamily=BRAND_MED, va="center")
    run = data["run"]
    fig.text(
        0.975,
        0.938,
        f"Jev ({data['model']}), TypeSafe AI",
        fontsize=19,
        color=INK,
        fontfamily=BRAND_MED,
        ha="right",
        va="center",
    )
    fig.text(
        0.975,
        0.900,
        f"{sum(c['test'] for c in data['item_counts'].values()):,} test items · {data['source_patches']} agent patches · "
        f"{run['latency_ms_p50']:.0f} ms median · ${run['total_cost_usd']:.2f} total",
        fontsize=13,
        color=INK2,
        ha="right",
        va="center",
        fontfamily=MONO,
    )
    fig.text(
        0.055,
        0.075,
        "Answers verified by the hidden test suites of VulcanBench Frontier v4, not by another model.",
        fontsize=12,
        color=MUTED,
        va="center",
    )
    fig.text(
        0.055,
        0.048,
        "*Style is the exception: it scores agreement with the Muse and Grok code-quality panel, which is opinion, not ground truth.",
        fontsize=12,
        color=MUTED,
        va="center",
    )
    fig.text(
        0.055, 0.016, "vulcanbench.com", fontsize=12, color=INK2, fontfamily=BRAND_MED, va="center"
    )

    border = FancyBboxPatch(
        (0.004, 0.004),
        0.992,
        0.992,
        transform=fig.transFigure,
        boxstyle="round,pad=0,rounding_size=0.012",
        linewidth=1.2,
        edgecolor=GRID,
        facecolor="none",
    )
    fig.patches.append(border)
    fig.savefig(args.out, facecolor=SURFACE)
    print(f"wrote {args.out} ({W}x{H})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

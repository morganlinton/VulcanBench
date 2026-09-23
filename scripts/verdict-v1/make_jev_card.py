#!/usr/bin/env python3
"""Render the shareable VulcanBench Verdict v1 card for Jev.

Three panels: accuracy against the majority-answer floor at a cutoff fitted
on the development split, the distribution of the probabilities Jev actually
produced, and the ranking quality that survives having no usable cutoff.
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
FAIL = "#b34a3a"
W, H, DPI = 2560, 1440, 200
FAMILIES = [
    ("patch-verdict", "Does this patch\npass every test?"),
    ("patch-regression", "Does it break\nan existing test?"),
    ("patch-outcome", "Which of four\noutcomes?"),
    ("quality-preference", "Which patch is more\nmaintainable?*"),
]


def decided_accuracy(metrics: dict) -> float:
    """Accuracy at the fitted cutoff where there is one, else the top answer."""
    return metrics.get("accuracy_at_threshold", metrics["accuracy"]) * 100


def panel_accuracy(ax, data: dict) -> None:
    jev = data["results"][data["model"]]["families"]
    floor = data["results"]["majority_floor"]["families"]
    x = range(len(FAMILIES))
    width = 0.38
    series = (
        (-width / 2, floor, FLOOR, "Always answer the most common label"),
        (width / 2, jev, JEV, "Jev, at a cutoff fitted on held-out items"),
    )
    for offset, source, color, label in series:
        values = [decided_accuracy(source[family]) for family, _ in FAMILIES]
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
        "Accuracy against the majority answer",
        fontsize=16,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
    )
    ax.legend(frameon=False, fontsize=12.5, loc="lower right", bbox_to_anchor=(1, 1.0), ncols=2)


def panel_distribution(ax, data: dict) -> None:
    histogram = data["diagnostics"]["patch_verdict_p_true_histogram"]
    step = histogram["step"]
    edges = [float(edge) for edge in histogram["bins"]]
    passes = [counts["passes"] for counts in histogram["bins"].values()]
    fails = [counts["fails"] for counts in histogram["bins"].values()]
    ax.bar(
        edges, passes, width=step * 0.92, align="edge", color=JEV, label="really passes", zorder=3
    )
    ax.bar(
        edges,
        fails,
        width=step * 0.92,
        align="edge",
        bottom=passes,
        color=FAIL,
        label="really fails",
        zorder=3,
    )
    ax.axvline(0.5, color=INK2, linewidth=1.4, linestyle=(0, (4, 3)), zorder=4)
    ax.text(
        0.49,
        ax.get_ylim()[1] * 0.80,
        "0.5, where a yes\nwould begin",
        fontsize=11.5,
        color=INK2,
        ha="right",
    )
    top = data["results"][data["model"]]["families"]["patch-verdict"]["p_true_max"]
    ax.set_xlim(0, 0.62)
    ax.set_xlabel(
        f"Jev's stated probability that the patch passes (highest: {top:.2f})",
        fontsize=12,
        color=INK2,
    )
    ax.set_ylabel("patches", fontsize=12, color=INK2)
    ax.set_title(
        "It never claims a patch passes, though 63% of them do",
        fontsize=15,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
    )
    ax.legend(frameon=False, fontsize=12, loc="upper left")


def panel_auroc(ax, data: dict) -> None:
    jev = data["results"][data["model"]]["families"]
    rows = [
        (label.replace("\n", " ").rstrip("*"), jev[family]["auroc"]) for family, label in FAMILIES
    ]
    rows = [(label, value) for label, value in rows if value is not None]
    y = range(len(rows))
    ax.barh(list(y), [value for _, value in rows], height=0.52, color=JEV, zorder=3)
    for i, (_, value) in enumerate(rows):
        ax.text(
            value + 0.012, i, f"{value:.2f}", va="center", fontsize=13, color=INK, fontfamily=MONO
        )
    ax.axvline(0.5, color=INK2, linewidth=1.4, linestyle=(0, (4, 3)), zorder=4)
    ax.text(0.51, -0.42, "0.5, coin flip", fontsize=11.5, color=INK2)
    ax.set_yticks(list(y))
    ax.set_yticklabels([label for label, _ in rows], fontsize=11.5, color=INK2)
    ax.set_ylim(len(rows) - 0.4, -0.75)
    ax.set_xlim(0, 1.08)
    ax.set_xlabel(
        "AUROC: does it rank the right answer higher, whatever the cutoff",
        fontsize=12,
        color=INK2,
    )
    ax.set_title(
        "The ranking underneath carries real signal",
        fontsize=15,
        color=INK,
        fontfamily=BRAND_MED,
        pad=14,
        loc="left",
    )
    ax.grid(axis="y", visible=False)
    ax.grid(axis="x", color=GRID, linewidth=0.9, zorder=0)


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
        2, 2, left=0.055, right=0.975, top=0.785, bottom=0.175, hspace=0.66, wspace=0.28
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
    panel_distribution(axes[1], data)
    panel_auroc(axes[2], data)

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
        f"{sum(c['test'] for c in data['item_counts'].values()):,} test items · "
        f"{data['source_patches']} agent patches · {run['latency_ms_p50']:.0f} ms median · "
        f"${run['total_cost_usd']:.2f} total",
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

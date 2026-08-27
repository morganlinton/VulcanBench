"""Report No. 20 hero card: the one-stat lead image for X.

A single 16:9 card, Muse Spark 1.2 raw API vs the Pi harness at xhigh effort.
All six answer-key Pi cells were replaced by sandbox-confined, audited-clean
reruns, so the headline needs no asterisk: 52% vs 78%, harness worth 26 points.

Writes vulcanbench-v3-musespark-hero.png next to the report.
Usage: python scripts/rankings-chart/make_musespark_hero.py
"""

from __future__ import annotations

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from _common import BRAND, BRAND_MED, INK, INK2, MUTED, SANS, SURFACE, register_fonts

HERE = Path(__file__).resolve().parent
REPORT = HERE.parent.parent / "docs" / "results" / "v3-musespark-harness-2026-08"
OUT = REPORT / "vulcanbench-v3-musespark-hero.png"
LOGO = HERE / "vb_logo_rounded.png"

API_COLOR = INK
PI_COLOR = "#10b981"


def main() -> None:
    register_fonts()
    fig = plt.figure(figsize=(16, 9), dpi=160)
    fig.patch.set_facecolor(SURFACE)
    ax = fig.add_axes((0, 0, 1, 1))
    ax.set_axis_off()
    ax.set_xlim(0, 1)
    ax.set_ylim(0, 1)

    tx = 0.066
    try:
        logo = plt.imread(str(LOGO))
        h = 0.085
        w = h * 9 / 16
        ax.imshow(logo, extent=(tx, tx + w, 0.86, 0.86 + h), aspect="auto", zorder=5)
        wx = tx + w + 0.015
    except Exception:
        wx = tx
    ax.text(wx, 0.918, "VulcanBench", family=BRAND, fontsize=30, color=INK, va="center")
    ax.text(wx, 0.874, "Report No. 20", family=BRAND_MED, fontsize=15, color=MUTED, va="center")

    ax.text(
        0.5,
        0.74,
        "One model. Two harnesses.",
        family=BRAND,
        fontsize=34,
        color=INK,
        ha="center",
        va="center",
    )
    ax.text(
        0.5,
        0.675,
        "Muse Spark 1.2 at xhigh effort, pass@1 on 23 real merged PRs",
        family=SANS,
        fontsize=17,
        color=INK2,
        ha="center",
        va="center",
    )

    lx, rx = 0.29, 0.71
    ax.text(lx, 0.44, "52%", family=BRAND, fontsize=150, color=API_COLOR, ha="center", va="center")
    ax.text(rx, 0.44, "78%", family=BRAND, fontsize=150, color=PI_COLOR, ha="center", va="center")
    ax.text(lx, 0.265, "Raw API", family=BRAND_MED, fontsize=24, color=API_COLOR, ha="center")
    ax.text(rx, 0.265, "Pi harness", family=BRAND_MED, fontsize=24, color=PI_COLOR, ha="center")
    ax.text(
        lx,
        0.225,
        "Meta Model API, uniform agent loop",
        family=SANS,
        fontsize=14,
        color=MUTED,
        ha="center",
        va="top",
    )
    ax.text(
        rx,
        0.225,
        "open-source agent, same model, Docker tests\n(six answer-key cells replaced by confined, audited-clean reruns)",
        family=SANS,
        fontsize=14,
        color=MUTED,
        ha="center",
        va="top",
    )
    ax.plot([0.5, 0.5], [0.30, 0.52], color="#e2e1db", lw=1.5, zorder=0)

    ax.text(
        0.5,
        0.115,
        "Same model. The harness is worth 26 points.",
        family=BRAND,
        fontsize=26,
        color=INK,
        ha="center",
        va="center",
    )

    ax.text(
        0.5,
        0.045,
        "VulcanBench v3   ·   hidden-test grading   ·   Docker verifier   ·   "
        "one attempt per cell   ·   Pi agent is host-run",
        family=SANS,
        fontsize=13,
        color=MUTED,
        ha="center",
        va="center",
    )

    fig.savefig(OUT, facecolor=SURFACE)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()

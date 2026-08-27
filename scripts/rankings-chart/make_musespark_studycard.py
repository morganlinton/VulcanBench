"""Report No. 20 harness-study card: Muse Spark 1.2 in Pi vs a bare-bones harness.

Two panels in the VulcanBench Harness Study format:
  Left  - pass@1 by effort, Pi vs the bare-bones API loop, +/-1 stderr whiskers.
  Right - where Muse Spark's raw-API best (87.0, low) lands on the v3
          best-effort leaderboard. Pi (model plus agent) is charted in the left
          panel only: it is not a board entry, so it does not get a board bar.

Reads docs/results/v3-musespark-harness-2026-08/v3-musespark-harness-2026-08.json
and writes report20-musespark-pi.png next to it.
Usage: python scripts/rankings-chart/make_musespark_studycard.py
"""

from __future__ import annotations

import json
from math import sqrt
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from _common import BRAND, BRAND_MED, GRID, INK, INK2, MUTED, SANS, SURFACE, register_fonts

HERE = Path(__file__).resolve().parent
REPORT = HERE.parent.parent / "docs" / "results" / "v3-musespark-harness-2026-08"
DATA = REPORT / "v3-musespark-harness-2026-08.json"
OUT = REPORT / "report20-musespark-pi.png"
LOGO = HERE / "vb_logo_rounded.png"

API_COLOR = INK
PI_COLOR = "#10b981"
FIELD = "#c7c3ba"
EFFORTS = ["low", "high", "extra-high"]
LBL = {"low": "low", "high": "high", "extra-high": "xhigh"}

# Field models only. Pi is model plus agent and never appears on this board
# (the model card's rule); the left panel carries the harness comparison.
BOARD = [
    ("Grok 4.5", 89.9),
    ("Claude Fable 5", 89.5),
    ("DeepSeek V4-Flash", 88.4),
    ("DeepSeek V4 Pro", 87.0),
    ("GPT-5.6 Terra", 87.0),
    ("Claude Opus 5", 87.0),
    ("Grok 4.6", 87.0),
    ("GPT-5.6 Sol", 87.0),
    ("GPT-5.6 Luna", 85.5),
    ("Qwen3.8-27B", 82.6),
    ("Qwen3.8-Max", 81.2),
    ("GLM 5.3 (raw API)", 78.3),
    ("Claude Haiku 4.5", 76.2),
    ("Kimi K3", 73.7),
]
API_BEST = 87.0


def stderr_pct(solved: int, n: int) -> float:
    p = solved / n
    return sqrt(p * (1 - p) / n) * 100


def main() -> None:  # noqa: PLR0915 (single linear chart layout)
    register_fonts()
    data = json.loads(DATA.read_text())
    api, pi = data["api"], data["pi"]

    def series(d):
        return (
            [d[e]["passat1"] for e in EFFORTS],
            [stderr_pct(d[e]["solved"], d[e]["n"]) for e in EFFORTS],
        )

    api_y, api_se = series(api)
    pi_y, pi_se = series(pi)

    fig = plt.figure(figsize=(16, 9), dpi=160)
    fig.patch.set_facecolor(SURFACE)

    hax = fig.add_axes((0, 0.85, 1, 0.15))
    hax.axis("off")
    tx = 0.062
    try:
        logo = plt.imread(str(LOGO))
        hax.imshow(
            logo,
            extent=(tx, tx + 0.05, 0.24, 0.86),
            transform=hax.transAxes,
            aspect="auto",
            zorder=5,
        )
        wx = tx + 0.066
    except Exception:
        wx = tx
    hax.text(
        wx,
        0.64,
        "VulcanBench",
        family=BRAND,
        fontsize=26,
        color=INK,
        va="center",
        transform=hax.transAxes,
    )
    hax.text(
        wx,
        0.26,
        "Technical Report No. 20   ·   Harness Study No. 04",
        family=BRAND_MED,
        fontsize=13,
        color=MUTED,
        va="center",
        transform=hax.transAxes,
    )
    hax.text(
        0.938,
        0.5,
        "Muse Spark 1.2 in Pi\nvs. a bare-bones harness",
        family=BRAND_MED,
        fontsize=14.5,
        color=INK2,
        va="center",
        ha="right",
        transform=hax.transAxes,
        linespacing=1.3,
    )

    fig.text(
        0.062,
        0.795,
        "Through its raw API, Muse Spark 1.2 ties the 87% frontier cluster. Pi lifts the same",
        family=SANS,
        fontsize=16.5,
        color=INK,
        va="center",
    )
    fig.text(
        0.062,
        0.758,
        "model 8.7 points at low; the clean xhigh lift is 17 points (39 as graded).",
        family=SANS,
        fontsize=16.5,
        color=INK,
        va="center",
    )

    ax = fig.add_axes((0.062, 0.17, 0.40, 0.47))
    _grid(ax)
    x = [0, 1, 2]
    ax.errorbar(
        x,
        pi_y,
        yerr=pi_se,
        color=PI_COLOR,
        lw=3.2,
        marker="o",
        ms=11,
        capsize=6,
        elinewidth=2,
        capthick=2,
        zorder=5,
        label="Pi harness (as graded)",
    )
    pi_clean = [pi[e]["passat1_clean"] for e in EFFORTS]
    ax.plot(
        x,
        pi_clean,
        "--o",
        color=PI_COLOR,
        lw=1.8,
        ms=8,
        mfc=SURFACE,
        zorder=5,
        label="Pi, answer-key cells dropped",
    )
    ax.errorbar(
        x,
        api_y,
        yerr=api_se,
        color=API_COLOR,
        lw=3.2,
        marker="o",
        ms=11,
        capsize=6,
        elinewidth=2,
        capthick=2,
        zorder=5,
        label="Bare-bones API loop",
    )
    label_box = dict(facecolor=SURFACE, edgecolor="none", pad=1.6)
    # Pi labels sit in the unused band above 100. API labels sit to the right
    # of each point so they never land on a grid line or the x-axis.
    for xi, yi in zip(x, pi_y, strict=True):
        ax.text(
            xi,
            108.5,
            f"{yi:.1f}",
            ha="center",
            family=BRAND_MED,
            fontsize=13.5,
            color=PI_COLOR,
            bbox=label_box,
            zorder=6,
            clip_on=False,
        )
    for xi, yi in zip(x, api_y, strict=True):
        ax.text(
            xi + 0.22,
            yi - 4.8,
            f"{yi:.1f}",
            ha="left",
            va="top",
            family=BRAND_MED,
            fontsize=13.5,
            color=API_COLOR,
            bbox=label_box,
            zorder=6,
            clip_on=False,
        )
    for xi, yi in ((1, pi_clean[1]), (2, pi_clean[2])):
        ax.text(
            xi - 0.22,
            yi - 1.2,
            f"{yi:.1f}",
            ha="right",
            va="top",
            family=BRAND_MED,
            fontsize=12,
            color=PI_COLOR,
            bbox=label_box,
            zorder=6,
        )
    ax.set_xticks(x)
    ax.set_xticklabels([LBL[e] for e in EFFORTS], family=SANS, fontsize=14, color=INK2)
    ax.set_xlim(-0.35, 2.45)
    ax.set_ylim(42, 118)
    ax.set_yticks([50, 60, 70, 80, 90, 100])
    ax.set_yticklabels(["50", "60", "70", "80", "90", "100"], family=SANS, fontsize=11, color=MUTED)
    ax.set_xlabel("reasoning effort", family=SANS, fontsize=12.5, color=INK2)
    ax.set_title("pass@1 by effort", family=BRAND_MED, fontsize=15, color=INK, loc="left", pad=10)
    ax.legend(
        loc="lower left",
        frameon=False,
        fontsize=12,
        handlelength=1.6,
        bbox_to_anchor=(0.02, 0.02),
    )

    ax2 = fig.add_axes((0.58, 0.12, 0.36, 0.58))
    ax2.set_facecolor(SURFACE)
    for s in ("top", "right", "left"):
        ax2.spines[s].set_visible(False)
    ax2.spines["bottom"].set_color(GRID)
    ax2.tick_params(length=0)
    # Muse Spark's raw-API best joins the field at its sorted spot (head of
    # the 87.0 tie). Pi is deliberately absent: not a board entry.
    rows = [(nm, val, FIELD, False) for nm, val in BOARD]
    spot = next(i for i, (_nm, val, _c, _h) in enumerate(rows) if val <= API_BEST)
    rows.insert(spot, ("Muse Spark 1.2 raw API", API_BEST, API_COLOR, True))
    n = len(rows)
    ys = list(range(n, 0, -1))
    base = 70.0
    for (name, val, color, hi), y in zip(rows, ys, strict=True):
        ax2.barh(y, val - base, left=base, height=0.62, color=color, zorder=3)
        ax2.text(
            base - 0.55,
            y,
            name,
            ha="right",
            va="center",
            family=BRAND_MED if hi else SANS,
            fontsize=10.2,
            color=INK if hi else INK2,
        )
        ax2.text(
            min(val + 0.9, 99.4),
            y,
            f"{val:.1f}",
            ha="left",
            va="center",
            family=BRAND_MED if hi else SANS,
            fontsize=10,
            color=INK if hi else MUTED,
            bbox=dict(facecolor=SURFACE, edgecolor="none", pad=0.8),
            zorder=4,
        )
    ax2.set_xlim(base, 101.5)
    ax2.set_ylim(0.2, n + 2.4)
    ax2.set_yticks([])
    ax2.set_xticks([70, 80, 90, 100])
    ax2.set_xticklabels(["70", "80", "90", "100"], family=SANS, fontsize=11, color=MUTED)
    ax2.set_title(
        "Muse Spark 1.2 raw API on the v3 board  (pass@1 %)",
        family=BRAND_MED,
        fontsize=15,
        color=INK,
        loc="left",
        pad=18,
    )

    fig.text(
        0.062,
        0.072,
        "VulcanBench v3: 23 post-cutoff merged PRs, hidden deterministic tests, Docker verifier. "
        "One attempt per cell for Muse Spark 1.2; whiskers are +/-1 binomial stderr.",
        family=SANS,
        fontsize=10.5,
        color=MUTED,
        va="center",
    )
    fig.text(
        0.062,
        0.038,
        "Board entries are raw-API / uniform-loop runs; Pi (model plus agent) is charted left, never "
        "on the board. Dashed Pi line drops six answer-key cells. Pi cash was under-metered; not quoted.",
        family=SANS,
        fontsize=10.5,
        color=MUTED,
        va="center",
    )
    fig.text(
        0.938,
        0.072,
        "vulcanbench.com",
        family=BRAND_MED,
        fontsize=12.5,
        color=INK2,
        va="center",
        ha="right",
    )

    fig.savefig(OUT, facecolor=SURFACE)
    print(f"wrote {OUT}")


def _grid(ax: plt.Axes) -> None:
    ax.set_facecolor(SURFACE)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)
    for s in ("left", "bottom"):
        ax.spines[s].set_color(GRID)
    ax.tick_params(length=0)
    ax.set_axisbelow(True)
    ax.yaxis.grid(True, color=GRID, lw=1)
    ax.set_ylabel("pass@1  (%)", family=SANS, fontsize=12.5, color=INK2)


if __name__ == "__main__":
    main()

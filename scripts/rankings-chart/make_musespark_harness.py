"""Report No. 20 chart: Muse Spark 1.2 raw API vs the Pi harness.

Two panels in VulcanBench branding:
  1. Effort curves, pass@1 by effort for each harness (inverted vs flat).
  2. Failure composition, solved / wrong / unfinished per harness x effort
     (the raw API fails by timeout, Pi by wrong answer).

Reads docs/results/v3-musespark-harness-2026-08/v3-musespark-harness-2026-08.json
and writes vulcanbench-v3-musespark-harness.png next to it.

Usage: python scripts/rankings-chart/make_musespark_harness.py
"""

from __future__ import annotations

import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from _common import BRAND, BRAND_MED, GRID, INK, INK2, MUTED, SANS, SURFACE, register_fonts

HERE = Path(__file__).resolve().parent
REPORT = HERE.parent.parent / "docs" / "results" / "v3-musespark-harness-2026-08"
DATA = REPORT / "v3-musespark-harness-2026-08.json"
OUT = REPORT / "vulcanbench-v3-musespark-harness.png"
LOGO = HERE / "vb_logo_rounded.png"

API_COLOR = INK  # raw API, VulcanBench uniform loop
PI_COLOR = "#10b981"  # Pi harness (emerald, the dashboard accent)
SOLVED = "#10b981"
WRONG = "#c9a24a"
TIMEOUT = "#c2554d"
EFFORTS = ["low", "high", "extra-high"]
EFF_LABEL = {"low": "low", "high": "high", "extra-high": "xhigh"}


def main() -> None:  # noqa: PLR0915 (single linear chart layout)
    register_fonts()
    data = json.loads(DATA.read_text())
    api, pi = data["api"], data["pi"]

    fig = plt.figure(figsize=(16, 9), dpi=160)
    fig.patch.set_facecolor(SURFACE)
    gs = fig.add_gridspec(
        2,
        2,
        height_ratios=[0.22, 1.0],
        hspace=0.32,
        wspace=0.18,
        left=0.065,
        right=0.965,
        top=0.97,
        bottom=0.11,
    )

    hax = fig.add_subplot(gs[0, :])
    hax.set_axis_off()
    hax.set_xlim(0, 1)
    hax.set_ylim(0, 1)
    box = hax.get_position()
    fig_w, fig_h = fig.get_size_inches()
    phys_w = box.width * fig_w
    phys_h = box.height * fig_h
    chip_h = 0.78
    chip_w = chip_h * phys_h / phys_w
    try:
        logo = plt.imread(str(LOGO))
        hax.imshow(
            logo,
            extent=(0.0, chip_w, 0.11, 0.11 + chip_h),
            transform=hax.transAxes,
            aspect="auto",
            zorder=5,
            clip_on=False,
        )
        tx = chip_w + 0.014
    except Exception:
        tx = 0.0
    hax.text(
        tx,
        0.66,
        "VulcanBench",
        family=BRAND,
        fontsize=26,
        color=INK,
        va="center",
        transform=hax.transAxes,
        clip_on=False,
        zorder=6,
    )
    hax.text(
        tx,
        0.20,
        "Report No. 20   ·   Muse Spark 1.2: model versus harness   ·   suite v3   ·   23 tasks   ·   1 attempt/cell",
        family=BRAND_MED,
        fontsize=12.5,
        color=INK2,
        va="center",
        transform=hax.transAxes,
        clip_on=False,
        zorder=6,
    )
    hax.text(
        0.999,
        0.66,
        "pass@1 by reasoning effort",
        family=SANS,
        fontsize=13,
        color=MUTED,
        va="center",
        ha="right",
        transform=hax.transAxes,
        clip_on=False,
        zorder=6,
    )
    hax.set_xlim(0, 1)
    hax.set_ylim(0, 1)

    ax = fig.add_subplot(gs[1, 0])
    _style(ax)
    x = list(range(3))
    api_y = [api[e]["passat1"] for e in EFFORTS]
    pi_y = [pi[e]["passat1"] for e in EFFORTS]
    api_err = [_stderr_pts(y, api[e]["n"]) for y, e in zip(api_y, EFFORTS, strict=True)]
    pi_err = [_stderr_pts(y, pi[e]["n"]) for y, e in zip(pi_y, EFFORTS, strict=True)]
    ax.errorbar(
        x,
        pi_y,
        yerr=pi_err,
        fmt="-o",
        color=PI_COLOR,
        lw=3.2,
        ms=11,
        capsize=5,
        elinewidth=1.4,
        zorder=4,
        label="Pi harness (confined)",
    )
    ax.errorbar(
        x,
        api_y,
        yerr=api_err,
        fmt="-o",
        color=API_COLOR,
        lw=3.2,
        ms=11,
        capsize=5,
        elinewidth=1.4,
        zorder=4,
        label="Raw API (uniform loop)",
    )
    for xi, yi in zip(x, pi_y, strict=True):
        ax.text(
            xi,
            yi + 3.4,
            f"{yi:.1f}%",
            ha="center",
            family=BRAND_MED,
            fontsize=13,
            color=PI_COLOR,
            bbox=dict(facecolor=SURFACE, edgecolor="none", pad=1.4),
            zorder=6,
        )
    for xi, yi in zip(x, api_y, strict=True):
        ax.text(
            xi + 0.12,
            yi - 1.0,
            f"{yi:.1f}%",
            ha="left",
            va="top",
            family=BRAND_MED,
            fontsize=13,
            color=API_COLOR,
            bbox=dict(facecolor=SURFACE, edgecolor="none", pad=1.4),
            zorder=6,
        )
    ax.set_xticks(x)
    ax.set_xticklabels([EFF_LABEL[e] for e in EFFORTS], family=SANS, fontsize=13, color=INK2)
    ax.set_ylim(38, 104)
    ax.set_yticks([40, 50, 60, 70, 80, 90, 100])
    ax.set_yticklabels(
        ["40", "50", "60", "70", "80", "90", "100"], family=SANS, fontsize=11, color=MUTED
    )
    ax.set_ylabel("pass@1  (%)", family=SANS, fontsize=12, color=INK2)
    ax.set_xlabel("reasoning effort", family=SANS, fontsize=12, color=INK2)
    ax.legend(
        loc="lower left",
        frameon=False,
        fontsize=11.5,
        ncol=1,
        handlelength=1.6,
    )
    ax.set_title(
        "Same model: one curve slides, one collapses",
        family=BRAND_MED,
        fontsize=15,
        color=INK,
        pad=12,
        loc="left",
    )
    ax.annotate(
        "",
        xy=(2, pi_y[2]),
        xytext=(2, api_y[2]),
        arrowprops=dict(arrowstyle="<->", color=MUTED, lw=1.3),
    )
    ax.text(
        1.95,
        44.5,
        f"+{pi_y[2] - api_y[2]:.1f} pts",
        ha="right",
        va="center",
        family=BRAND_MED,
        fontsize=12,
        color=INK2,
        bbox=dict(facecolor=SURFACE, edgecolor="none", pad=1.4),
    )

    ax2 = fig.add_subplot(gs[1, 1])
    _style(ax2)
    bars = [("API", api, API_COLOR), ("Pi", pi, PI_COLOR)]
    xs, labels, group_centers = [], [], []
    pos = 0.0
    for _gi, (_hname, d, _c) in enumerate(bars):
        start = pos
        for e in EFFORTS:
            s, w, t = d[e]["solved"], d[e]["wrong"], d[e]["timeout"]
            ax2.bar(pos, s, 0.72, color=SOLVED, zorder=3)
            ax2.bar(pos, w, 0.72, bottom=s, color=WRONG, zorder=3)
            ax2.bar(pos, t, 0.72, bottom=s + w, color=TIMEOUT, zorder=3)
            ax2.text(
                pos,
                s * 0.52,
                f"{s}",
                ha="center",
                va="center",
                family=BRAND_MED,
                fontsize=12,
                color="#ffffff",
                bbox=dict(facecolor=SOLVED, edgecolor="none", pad=1.2),
            )
            xs.append(pos)
            labels.append(EFF_LABEL[e])
            pos += 1.0
        group_centers.append((start + pos - 1.0) / 2)
        pos += 0.9
    ax2.set_xticks(xs)
    ax2.set_xticklabels(labels, family=SANS, fontsize=11, color=INK2)
    for gc, (hname, _d, c) in zip(group_centers, bars, strict=True):
        ax2.text(
            gc,
            -2.3,
            "Raw API" if hname == "API" else "Pi harness",
            ha="center",
            family=BRAND_MED,
            fontsize=12.5,
            color=c,
        )
    ax2.set_ylim(0, 24)
    ax2.yaxis.grid(False)
    ax2.set_yticks([0, 5, 10, 15, 20, 23])
    ax2.set_yticklabels(["0", "5", "10", "15", "20", "23"], family=SANS, fontsize=11, color=MUTED)
    ax2.set_ylabel("runs (of 23)", family=SANS, fontsize=12, color=INK2)
    ax2.set_title(
        "How each harness fails: timeouts vs wrong answers",
        family=BRAND_MED,
        fontsize=15,
        color=INK,
        pad=26,
        loc="left",
    )
    handles = [
        plt.Rectangle((0, 0), 1, 1, color=SOLVED),
        plt.Rectangle((0, 0), 1, 1, color=WRONG),
        plt.Rectangle((0, 0), 1, 1, color=TIMEOUT),
    ]
    ax2.legend(
        handles,
        ["solved", "wrong answer", "unfinished (timeout)"],
        loc="lower right",
        bbox_to_anchor=(1.0, 1.005),
        ncol=3,
        frameon=False,
        fontsize=11,
        handlelength=1.2,
        columnspacing=1.4,
    )

    fig.text(
        0.065,
        0.047,
        "Same Muse Spark 1.2, same 23 post-cutoff PRs, hidden-test grading, n=23 per column, "
        "whiskers +/-1 binomial stderr, one attempt per cell. Six answer-key Pi cells replaced "
        "by confined reruns, audited clean.",
        family=SANS,
        fontsize=10,
        color=MUTED,
        va="center",
    )
    fig.text(
        0.065,
        0.024,
        "Both tracks metered. Uniform loop 56.36 USD; the original Pi sweep under-metered cash "
        "(fixed in v0.9.1; the six rerun cells alone metered 30.25 USD), so a whole-column Pi "
        "cost is not quoted.",
        family=SANS,
        fontsize=10,
        color=MUTED,
        va="center",
    )
    fig.savefig(OUT, facecolor=SURFACE)
    print(f"wrote {OUT}")


def _stderr_pts(passat1_pct: float, n: int) -> float:
    """+/-1 binomial stderr of a pass@1 percentage, in points."""
    p = passat1_pct / 100.0
    return 100.0 * (p * (1.0 - p) / n) ** 0.5


def _style(ax: plt.Axes) -> None:
    ax.set_facecolor(SURFACE)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)
    for s in ("left", "bottom"):
        ax.spines[s].set_color(GRID)
    ax.tick_params(length=0)
    ax.set_axisbelow(True)
    ax.yaxis.grid(True, color=GRID, lw=1)


if __name__ == "__main__":
    main()

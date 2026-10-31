"""GPT-6.1 Terra, Claude Sonnet 5.5 and Muse Wonder Full on VulcanBench Frontier v4.

Owner request 2026-10-31. Same compact 16:9 layout as the Terra card in
make_gpt61terra_vs_frontier_card.py: model as the title, combined score and
cost per task at every effort level, a short footnote. The per-level table,
minutes included, is written to the CSV beside the card.

Source, nothing re-run or re-judged: the published leaderboard table
(VulcanBenchCOM assets/data/swe-v4-board.csv), which carries each model's
per-level rows (Terra v3.29, Sonnet v3.23, Muse Wonder Full v3.26). Its
sha256 is recorded in the card's JSON.

Muse Wonder Full ran in Muse Code's Contributor mode, which has no Max level,
so its line stops at Extra high and its Max cost slot reads n/a.

    python scripts/cii-v4-board/make_gpt61terra_vs_sonnet55_musewonder_card.py \
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
MODELS = ("gpt61terra", "sonnet55", "wonder")
BOARD_NAMES = {
    "gpt61terra": "GPT-6.1 Terra",
    "sonnet55": "Sonnet 5.5",
    "wonder": "Muse Wonder Full",
}
NAMES = {
    "gpt61terra": "GPT-6.1 Terra",
    "sonnet55": "Claude Sonnet 5.5",
    "wonder": "Muse Wonder Full",
}
# Lab colors (OpenAI green, Anthropic clay, Meta blue); Terra's own card uses blue, which
# would clash with Muse here. On the card paper the worst color-vision pair is clay and
# green, so markers and direct labels carry identity as well as color.
COLORS = {"gpt61terra": "#10A37F", "sonnet55": "#D97757", "wonder": "#0866FF"}
MARKERS = {"gpt61terra": "o", "sonnet55": "D", "wonder": "s"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"


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
                        "timeouts_zero": r["combined_timeouts_zero"] or "",
                        "protocol": r["protocol"],
                        "harness": r["harness"],
                    }
    for m in MODELS:
        have = [e for e in LEVELS if (m, e) in groups]
        require(have == list(LEVELS[: len(have)]) and have, f"{m} levels are not a prefix: {have}")
    return groups


def main():  # noqa: PLR0912, PLR0915, one linear figure
    p = argparse.ArgumentParser()
    p.add_argument("--board", type=Path, required=True)
    p.add_argument("--out", type=Path, default=OUTPUT)
    args = p.parse_args()
    groups = load(args.board)
    levels = {m: [e for e in LEVELS if (m, e) in groups] for m in MODELS}

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
        x += {"gpt61terra": 0.15, "sonnet55": 0.175, "wonder": 0}[m]

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

    # Combined score: all three overlap, so one shared scale.
    text(left, 3.0, "Combined score", 20, True, heading=True)
    text(left, 3.32, "higher is better  ·  whiskers ±1 se", 12, color=MUTED)
    span_h = 3.9
    pts = [groups[k] for k in groups]
    lo = math.floor(min(g["mean"] - g["se"] for g in pts)) - 1
    hi = math.ceil(max(g["mean"] + g["se"] for g in pts)) + 1
    ax = axis(0.085, 0.40, 3.7, span_h)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    ax.set_ylim(lo, hi)
    step = 2 if hi - lo <= 16 else 5
    ax.set_yticks(range(lo + (-lo) % step, hi + 1, step))
    # Lines that reach Max get end labels past the Max column; a line that stops short is
    # labelled above its first segment instead, so its label never sits where it has no point.
    full = [m for m in MODELS if len(levels[m]) == len(LEVELS)]
    end = {m: groups[m, "max"]["mean"] for m in full}
    gap = 0.06 * (hi - lo)
    placed = {}
    for m in sorted(full, key=end.get):
        placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
    shift = statistics.mean(placed.values()) - statistics.mean(end.values())
    dodge = {"gpt61terra": -0.06, "sonnet55": 0.0, "wonder": 0.06}
    for m in MODELS:
        xs = [LEVELS.index(e) + dodge[m] for e in levels[m]]
        ys = [groups[m, e]["mean"] for e in levels[m]]
        es = [groups[m, e]["se"] for e in levels[m]]
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
        if m in full:
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
        else:
            ax.text(
                xs[0] - 0.12,
                max(y + e for y, e in zip(ys[:2], es[:2], strict=True)) + 0.4,
                NAMES[m].replace("Claude ", ""),
                ha="left",
                va="center",
                fontsize=10.5,
                color=INK,
                fontfamily="Geist",
                weight=700,
                zorder=4,
                bbox={"facecolor": PAPER, "edgecolor": "none", "pad": 1.5},
            )

    # Cost per task.
    text(0.56, 3.0, "Cost per task", 20, True, heading=True)
    text(0.56, 3.32, "US dollars at list price  ·  lower is cheaper", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, span_h)
    shifts = {"gpt61terra": -0.27, "sonnet55": 0.0, "wonder": 0.27}
    top = max(g["usd"] for g in pts)
    ax.set_ylim(0, top * 1.18)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        idx = [LEVELS.index(e) for e in levels[m]]
        ys = [groups[m, e]["usd"] for e in levels[m]]
        ax.bar(
            [i + shifts[m] for i in idx],
            ys,
            width=0.25,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in zip(idx, ys, strict=True):
            ax.annotate(
                f"{y:.2f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8,
                fontfamily="IBM Plex Mono",
                color=INK,
            )
    for m in MODELS:
        for e in LEVELS[len(levels[m]) :]:
            ax.annotate(
                "n/a",
                (LEVELS.index(e) + shifts[m], 0),
                xytext=(0, 4),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8,
                fontfamily="IBM Plex Mono",
                color=MUTED,
            )

    short = {m: NAMES[m].replace("Claude ", "") for m in MODELS}
    partial = [
        (m, e) for m in MODELS for e in levels[m] if groups[m, e]["judged"] < groups[m, e]["of"]
    ]
    require(
        all(groups[k]["timeouts_zero"] for k in partial), "a partial point has no timeout figure"
    )
    by_model = {}
    for m, e in partial:
        by_model.setdefault(m, []).append(e.replace("-", " ").capitalize())
    notes = []
    for m, es in by_model.items():
        n = {groups[m, e]["judged"] for e in levels[m] if (m, e) in partial}
        require(len(n) == 1, "mixed partial counts")
        notes.append(f"{short[m]} {' and '.join(es)} {n.pop()} each (one timeout left out)")
    missing = [short[m] for m in MODELS if len(levels[m]) < len(LEVELS)]
    line1 = "Judged runs: 23 per point except " + "; ".join(notes) + "."
    if missing:
        line1 += f" {' and '.join(missing)} has no Max level in its mode."
    line2 = (
        "Code quality judges: Muse Spark 1.3 and Grok 4.6; for Muse Wonder Full, Claude Opus 5.5 "
        "and GPT-6.1 Sol. Harnesses differ: Codex, Claude Code, Muse Code."
    )
    text(left, 8.3, line1, 11, color=MUTED)
    text(left, 8.6, line2, 11, color=MUTED)

    args.out.mkdir(parents=True, exist_ok=True)
    out = args.out / "terra-vs-sonnet55-musewonder-full.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = args.out / "terra-vs-sonnet55-musewonder-full-efforts.csv"
    cols = [
        "model",
        "effort",
        "harness",
        "protocol",
        "judged",
        "of",
        "combined",
        "combined_se",
        "combined_timeouts_zero",
        "code_quality",
        "passed",
        "usd_per_task",
        "minutes",
    ]
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(cols)
        for m in MODELS:
            for e in levels[m]:
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
                        g["timeouts_zero"],
                        f"{g['code_quality']:.4f}",
                        g["passed"],
                        f"{g['usd']:.4f}",
                        f"{g['minutes']:.4f}",
                    ]
                )
    save(
        args.out / "terra-vs-sonnet55-musewonder-full.json",
        {
            "source": "VulcanBenchCOM assets/data/swe-v4-board.csv",
            "source_sha256": digest(args.board.read_bytes()),
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
            "footnote": [line1, line2],
        },
    )
    print(out)
    print(line1)


if __name__ == "__main__":
    main()

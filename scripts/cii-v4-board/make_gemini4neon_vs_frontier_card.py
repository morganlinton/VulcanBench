"""Gemini 4 Neon, Claude Opus 5.5 and GPT-6.2 Sol on VulcanBench Frontier v4.

The compact 16:9 card in the owner's minimal style (owner request 2026-11-21): the
models as the title, then two charts, combined score and cost per task at every effort
level, and one footnote that keeps judged run counts, pass counts, the judge panels and
the cost basis visible. The per-level table, minutes included, is written to the CSV
beside the card.

Antigravity offers Neon at Low, Medium, High and Extra high only (no Max), so Neon's
line stops one column early and the footnote says so.

Sources, nothing re-run or re-judged:

- Claude Opus 5.5 and GPT-6.2 Sol: their published per-level rows in the leaderboard
  table (VulcanBenchCOM assets/data/swe-v4-board.csv; Code quality v3.15 and v3.31, both
  Muse Spark 1.3 and Grok 4.6), whose sha256 is recorded in the card's JSON.
- Gemini 4 Neon: the Code quality v3.32 summary (Muse Spark 1.3 and Grok 4.6) and its run
  summaries in runs-effort-gemini4-neon-agy. Cost is the API-equivalent at Google's list
  price from the tokens Antigravity reported (Google AI Pro subscription runs). Low and
  Extra high are judged on 22 runs each: Grok's intent probes of submission-048 and
  submission-069 (both codeccore) were invalid (two malformed replies each), and the
  frozen summary leaves such a submission unpublished (owner choice, 2026-11-21, as in
  v3.7 and v3.17).

    python scripts/cii-v4-board/make_gemini4neon_vs_frontier_card.py --judging <v3.32 run directory> \\
        --board <VulcanBenchCOM checkout>/assets/data/swe-v4-board.csv
"""

from __future__ import annotations

import argparse
import csv
import json
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

OUTPUT = ROOT / "docs/results/swe-v4-gemini4-neon-2026-11"
RUNS = ROOT / "runs-effort-gemini4-neon-agy"
SPEC = "agy:gemini-4-neon"
STEM = "gemini4neon-vs-frontier"
MODELS = ("neon", "opus55", "gpt62sol")
NEON_LEVELS = ("low", "medium", "high", "extra-high")
BOARD_NAMES = {"opus55": "Opus 5.5", "gpt62sol": "GPT-6.2 Sol"}
# The board's passed_of counts judged runs, not runs, so run counts come from each
# model's published evidence bundle beside the board CSV.
BUNDLES = {"opus55": "swe-v4-opus55-v315", "gpt62sol": "swe-v4-gpt62-sol-v331"}
NAMES = {"neon": "Gemini 4 Neon", "opus55": "Claude Opus 5.5", "gpt62sol": "GPT-6.2 Sol"}
# Neon is the board's Google purple; Anthropic clay and OpenAI green as on every card.
# Markers and direct labels carry identity as well as color.
COLORS = {"neon": "#A142F4", "opus55": "#D97757", "gpt62sol": "#10A37F"}
MARKERS = {"neon": "D", "opus55": "s", "gpt62sol": "o"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
# Neon points judged on fewer than 23 runs (see the module docstring).
NEON_JUDGED = {"low": 22, "medium": 23, "high": 23, "extra-high": 22}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def levels(m):
    return NEON_LEVELS if m == "neon" else LEVELS


def load_groups(judging: Path, board: Path):
    """Per-level aggregates: Opus 5.5 and Sol from the published board, Neon from v3.32."""
    groups = {}
    runs_per_level = {}
    for m, bundle in BUNDLES.items():
        rows = json.loads((board.parent / bundle / "runs.json").read_text())["rows"]
        for e in LEVELS:
            runs_per_level[m, e] = sum(r["effort"] == e for r in rows)
    with board.open() as fh:
        for r in csv.DictReader(fh):
            for m, name in BOARD_NAMES.items():
                if r["model"] != name:
                    continue
                require((m, r["effort"]) not in groups, f"duplicate row {name} {r['effort']}")
                require(r["mean_usd"] != "", f"{name} {r['effort']} has no price")
                groups[m, r["effort"]] = {
                    "n": runs_per_level[m, r["effort"]],
                    "combined": {
                        "n": int(r["n"]),
                        "mean": float(r["combined_33"]),
                        "se": float(r["combined_33_se"]),
                    },
                    "code_quality": {"mean": float(r["code_quality"])},
                    "passed": int(r["passed"]),
                    "usd": {"mean": float(r["mean_usd"])},
                    "minutes": {"mean": float(r["mean_minutes"])},
                    "protocol": r["protocol"],
                }
    summary = json.loads((judging / "summary.json").read_text())
    require(
        summary["failed_panels"] == [] and summary["passing_panels"] == ["muse", "grok"],
        "a v3.32 panel failed",
    )
    for e in NEON_LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / e).glob("legacy-*/summary.json"))]
        require(len(runs) == 23, f"Neon {e} is not 23 runs")
        require(all(r["model"] == SPEC for r in runs), f"Neon {e} has another solver")
        g = summary["groups"][f"neon/{e}"]
        require(
            g["composite_v3"]["n"] == NEON_JUDGED[e],
            f"Neon {e} is {g['composite_v3']['n']} judged, expected {NEON_JUDGED[e]}",
        )
        groups["neon", e] = {
            "n": 23,
            "combined": g["composite_v3"],
            "code_quality": g["code_quality"],
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "usd": {
                "mean": statistics.mean(r["economics"]["api_equivalent_cost_usd"] for r in runs)
            },
            "minutes": {"mean": statistics.mean(r["duration_s"] for r in runs) / 60},
            "protocol": summary["protocol"],
        }
    for m in MODELS:
        for e in levels(m):
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "board_csv_sha256": digest(board.read_bytes()),
        "v3.32_summary_sha256": digest((judging / "summary.json").read_bytes()),
    }
    return groups, sources


def main():  # noqa: PLR0915, one linear figure
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True)
    parser.add_argument("--board", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    groups, sources = load_groups(args.judging, args.board)

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 9.0
    fig = plt.figure(figsize=(W, H), dpi=150, facecolor=PAPER)
    yf = lambda i: 1 - i / H  # noqa: E731

    def text(x, y, s, size=13, bold=False, heading=False, numeric=False, ha="left", color=INK):
        fam = (
            "IBM Plex Mono"
            if numeric
            else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium")
            if heading
            else "Geist"
        )
        wt = (
            (500 if bold else 400)
            if numeric
            else ((600 if bold else 500) if heading else (700 if bold else 400))
        )
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
    text(right, 0.65, "Frontier v4  ·  November 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    best = {m: max(levels(m), key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "Gemini 4 Neon vs Claude Opus 5.5 vs GPT-6.2 Sol", 30, True, heading=True)

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

    plot_h = 3.9
    text(left, 3.0, "Combined score", 20, True, heading=True)
    text(left, 3.32, "higher is better  ·  whiskers ±1 se", 12, color=MUTED)
    vals = [groups[m, e]["combined"] for m in MODELS for e in levels(m)]
    lo = math.floor(min(v["mean"] - v["se"] for v in vals)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 1
    ax = axis(0.085, 0.40, 3.7, plot_h)
    ax.set_xlim(-0.35, len(LEVELS) - 0.65)
    ax.set_ylim(lo, hi)
    ax.set_yticks(range(lo + lo % 2, hi + 1, 2) if hi - lo <= 16 else range(lo, hi + 1, 5))
    dodge = {"neon": 0.0, "opus55": -0.06, "gpt62sol": 0.06}
    for m in MODELS:
        xs = [LEVELS.index(e) + dodge[m] for e in levels(m)]
        ys = [groups[m, e]["combined"]["mean"] for e in levels(m)]
        es = [groups[m, e]["combined"]["se"] for e in levels(m)]
        ax.plot(xs, ys, color=COLORS[m], lw=2.2, zorder=2)
        ax.errorbar(
            xs,
            ys,
            yerr=es,
            fmt=MARKERS[m],
            color=COLORS[m],
            markersize=9,
            markeredgecolor=INK,
            markeredgewidth=0.6,
            ecolor=INK,
            elinewidth=0.8,
            capsize=3,
            zorder=3,
        )

    text(0.56, 3.0, "Cost per task", 20, True, heading=True)
    text(0.56, 3.32, "US dollars at list price  ·  lower is cheaper", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, plot_h)
    shifts = {"neon": -0.27, "opus55": 0.0, "gpt62sol": 0.27}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in levels(m))
    ax.set_ylim(0, top * 1.15)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        idx = [LEVELS.index(e) for e in levels(m)]
        ys = [groups[m, e]["usd"]["mean"] for e in levels(m)]
        ax.bar(
            [i + shifts[m] for i in idx],
            ys,
            width=0.26,
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
                rotation=90,
                fontsize=8.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    fewer_runs = [
        (m, e, groups[m, e]["n"]) for m in MODELS for e in levels(m) if groups[m, e]["n"] < 23
    ]
    fewer_judged = [
        (m, e, groups[m, e]["combined"]["n"], groups[m, e]["n"])
        for m in MODELS
        for e in levels(m)
        if groups[m, e]["combined"]["n"] < groups[m, e]["n"]
    ]
    require(
        fewer_runs == []
        and [(m, e, n) for m, e, n, _ in fewer_judged]
        == [("neon", "low", 22), ("neon", "extra-high", 22), ("opus55", "high", 22)],
        "run or judged counts changed; reword the footnote",
    )
    text(
        left,
        8.45,
        "23 runs per point; Neon has no Max. Judged on 22 of 23: Neon Low and Extra high, "
        "Opus 5.5 High. Judges: Muse Spark 1.3 and Grok 4.6. "
        "Cost is API-equivalent at list price. Harnesses differ.",
        11,
        color=MUTED,
    )

    args.output.mkdir(parents=True, exist_ok=True)
    out = args.output / f"{STEM}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = args.output / f"{STEM}-efforts.csv"
    with table.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(
            [
                "model",
                "effort",
                "n",
                "judged",
                "combined",
                "combined_se",
                "code_quality",
                "passed",
                "usd_per_task",
                "minutes",
                "protocol",
            ]
        )
        for m in MODELS:
            for e in levels(m):
                g = groups[m, e]
                w.writerow(
                    [
                        m,
                        e,
                        g["n"],
                        g["combined"]["n"],
                        f"{g['combined']['mean']:.4f}",
                        f"{g['combined']['se']:.4f}",
                        f"{g['code_quality']['mean']:.4f}",
                        g["passed"],
                        f"{g['usd']['mean']:.4f}",
                        f"{g['minutes']['mean']:.4f}",
                        g["protocol"],
                    ]
                )
    save(
        out.with_suffix(".json"),
        {
            "final": True,
            "sources": sources,
            "best": best,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in MODELS:
        print(f"  {NAMES[m]:<16} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

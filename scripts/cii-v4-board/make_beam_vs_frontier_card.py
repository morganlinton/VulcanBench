"""Reflection Beam, Claude Opus 5.5, GPT-6.1 Sol and GPT-6 Astra on VulcanBench Frontier v4.

A compact 16:9 card (owner request 2026-10-27): the model and version as the
title, then two charts, combined score and time per task at every effort
level, and one footnote line that keeps the judged run counts and the judge
panels visible. The per-level table it used to carry is written to the CSV
beside the card. Reflection publishes no Beam price, so the right panel shows
minutes, not cost.

The three frontier models finish within a few points of each other and Beam
20 to 50 points lower, so the combined score panel breaks its axis: the
frontier band gets its own zoomed scale above Beam's, and the break is drawn
and named on the card. On one shared scale the frontier lines would merge.

Sources, nothing re-run or re-judged:

- Claude Opus 5.5, GPT-6.1 Sol and GPT-6 Astra: their published per-level rows
  (Code quality v3.15, v3.18 and v3.4, all Muse Spark 1.3 and Grok 4.6), as on
  the earlier cards. Astra's cost is its report's central estimate at list price.
- Reflection Beam: the Code quality v3.28 summary (Muse Spark 1.3 and GPT-6.1
  Sol) and its run summaries in runs-effort-beam-mirror. Runs that changed no
  source code are not judged (owner decision 2026-10-26); at the levels where
  they occur the table also shows the combined score counting them as 0, as
  the board does for timeouts.

    python scripts/cii-v4-board/make_beam_vs_frontier_card.py --judging <v3.28 run directory>
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

PUBLISHED = ROOT / "docs/results/swe-v4-opus55-2026-09/opus55-astra-fable-efforts.csv"
SOL = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-efforts.csv"
SOL_COST = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-economics-efforts.csv"
ASTRA = ROOT / "docs/results/swe-v4-astra-fable51-2026-09/astra-vs-fable51-v34-efforts.csv"
ASTRA_COST = (
    ROOT / "docs/results/swe-v4-astra-fable51-2026-09/astra-vs-fable51-v34-economics-efforts.csv"
)
OUTPUT = ROOT / "docs/results/swe-v4-beam-2026-10"
RUNS = ROOT / "runs-effort-beam-mirror"
MODELS = ("beam", "opus55", "gpt61sol", "astra")
FRONTIER = ("opus55", "gpt61sol", "astra")
NAMES = {
    "beam": "Reflection Beam",
    "opus55": "Claude Opus 5.5",
    "gpt61sol": "GPT-6.1 Sol",
    "astra": "GPT-6 Astra",
}
# Beam takes a violet no lab on the board uses; Opus 5.5 and GPT-6.1 Sol keep their
# hues from the Sonnet 5.5 comparison card. A second OpenAI green fails the palette
# validator against Sol's (too dark or too gray), so Astra takes a dark gold. All
# four pass the all-pairs color-vision checks; markers, direct labels and the table
# carry identity as well as color.
COLORS = {"beam": "#7C4DFF", "opus55": "#8C3A1F", "gpt61sol": "#10A37F", "astra": "#B8860B"}
MARKERS = {"beam": "o", "opus55": "D", "gpt61sol": "^", "astra": "s"}
UNJUDGED_AS_ZERO = {"beam": "no-code runs as 0"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CLAUDE_MAIN = {"opus55": "claude-opus-5-5"}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = [v for v in values if v is not None]
    if not values:
        return {"n": 0, "mean": None, "se": None}
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return {"n": len(values), "mean": statistics.mean(values), "se": se}


def load_groups(judging: Path):
    """Per-level aggregates: Opus 5.5, GPT-6.1 Sol and GPT-6 Astra published, Beam from v3.28."""
    groups = {}
    with PUBLISHED.open() as handle:
        for r in csv.DictReader(handle):
            if r["model"] != "opus55":
                continue
            groups[r["model"], r["effort"]] = {
                "n": int(r["n"]),
                "combined": {
                    "n": int(r["judged"]),
                    "mean": float(r["combined"]),
                    "se": float(r["combined_se"]),
                },
                "code_quality": {"mean": float(r["code_quality"])},
                "passed": int(r["passed"]),
                "usd": {"mean": float(r["usd_per_task"])},
                "minutes": {"mean": float(r["minutes"])},
                "fb_runs": int(r["fallback_runs"]),
                "fb_share": float(r["opus48_reply_share_pct"]),
            }
    with SOL_COST.open() as handle:
        sol_cost = {r["effort"]: r for r in csv.DictReader(handle)}
    with SOL.open() as handle:
        for r in csv.DictReader(handle):
            require(r["model"] == "gpt61sol", f"unexpected row {r['model']}")
            c = sol_cost[r["effort"]]
            groups["gpt61sol", r["effort"]] = {
                "n": int(r["n"]),
                "combined": {
                    "n": int(r["n"]),
                    "mean": float(r["combined_v3"]),
                    "se": float(r["combined_v3_se"]),
                },
                "code_quality": {"mean": float(r["code_quality"])},
                "passed": int(r["passed"]),
                "usd": {"mean": float(c["mean_usd"])},
                "minutes": {"mean": float(r["minutes"])},
                "fb_runs": 0,
                "fb_share": None,
            }
    with ASTRA_COST.open() as handle:
        astra_cost = {r["effort"]: r for r in csv.DictReader(handle) if r["model"] == "astra"}
    with ASTRA.open() as handle:
        for r in csv.DictReader(handle):
            if r["model"] != "astra":
                continue
            c = astra_cost[r["effort"]]
            groups["astra", r["effort"]] = {
                "n": int(r["n"]),
                "combined": {
                    "n": int(r["n"]),
                    "mean": float(r["combined_v3"]),
                    "se": float(r["combined_v3_se"]),
                },
                "code_quality": {"mean": float(r["code_quality"])},
                "passed": int(r["passed"]),
                "usd": {"mean": float(c["mean_usd"])},
                "minutes": {"mean": float(r["minutes"])},
                "fb_runs": 0,
                "fb_share": None,
            }
    summary = json.loads((judging / "summary.json").read_text())
    require(
        summary["failed_panels"] == [] and len(summary["passing_panels"]) == 2,
        "a v3.28 panel failed",
    )
    population = json.loads(
        (judging.parent / "docs/results/swe-v4-beam-2026-10/comparison.json").read_text()
    )
    no_code = {r["run_id"] for r in population["rows"] if r["quality"] is None}
    for e in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / e).glob("legacy-*/summary.json"))]
        require(len(runs) == 23, f"Beam {e} is not 23 runs")
        g = summary["groups"][f"beam/{e}"]
        published = [
            r["published"]["composite_v3"]
            for r in summary["rows"]
            if r["effort"] == e and r.get("published")
        ]
        require(
            len(published) == g["composite_v3"]["n"],
            f"Beam {e} summary rows disagree with its group",
        )
        zero_runs = sum(
            r["effort"] == e and r["run_id"] in no_code for r in population["rows"]
        ) + sum(x["effort"] == e for x in population["excluded"])
        groups["beam", e] = {
            "n": 23,
            "combined": g["composite_v3"],
            "zero": {"mean": sum(published) / (len(published) + zero_runs), "runs": zero_runs}
            if zero_runs
            else None,
            "code_quality": g["code_quality"],
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "usd": {"mean": None},
            "tokens": {"mean": statistics.mean(r["total_tokens"] for r in runs)},
            "minutes": {"mean": statistics.mean(r["duration_s"] for r in runs) / 60},
            "fb_runs": 0,
            "fb_share": None,
        }
    for m in MODELS:
        for e in levels_of(m):
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "published_tables": {
            p.name: digest(p.read_bytes()) for p in (PUBLISHED, SOL, SOL_COST, ASTRA, ASTRA_COST)
        },
        "v3.28_summary_sha256": digest((judging / "summary.json").read_bytes()),
    }
    return groups, True, sources


def levels_of(m):
    return LEVELS


def main():  # noqa: PLR0915, one linear figure
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True)
    groups, final, sources = load_groups(parser.parse_args().judging)

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
    text(right, 0.65, "Frontier v4  ·  October 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    best = {m: max(levels_of(m), key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "Reflection Beam 1.0 (501B-A23B)", 30, True, heading=True)
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])

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
        label = f"{NAMES[m]}{' (with fallback)' if m in CLAUDE_MAIN else ''}"
        text(x + 0.016, 2.4, label, 13, True)
        x += {"beam": 0.19, "opus55": 0.27, "gpt61sol": 0.15, "astra": 0}[m]

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

    text(left, 3.0, "Combined score", 20, True, heading=True)
    # Broken axis: a zoomed band for the three frontier models over Beam's own band.
    top_h, gap_h, bot_h = 2.1, 0.14, 1.66
    upper = axis(0.085, 0.40, 3.7, top_h)
    lower = axis(0.085, 0.40, 3.7 + top_h + gap_h, bot_h)
    front = [groups[m, e]["combined"] for m in FRONTIER for e in LEVELS]
    up_lo = math.floor(min(v["mean"] - v["se"] for v in front))
    up_hi = math.ceil(max(v["mean"] + v["se"] for v in front)) + 1
    beam = [groups["beam", e] for e in LEVELS]
    lo_lo = 5 * math.floor((min(g["zero"]["mean"] for g in beam if g["zero"]) - 3) / 5)
    lo_hi = 5 * math.ceil((max(g["combined"]["mean"] + g["combined"]["se"] for g in beam) + 2) / 5)
    require(lo_hi < up_lo, "Beam and the frontier band overlap; the broken axis would hide data")
    text(
        left,
        3.32,
        f"higher is better  ·  axis broken between {lo_hi} and {up_lo}  ·  whiskers ±1 se",
        12,
        color=MUTED,
    )
    upper.set_ylim(up_lo, up_hi)
    upper.set_yticks(range(up_lo + up_lo % 2, up_hi + 1, 2))
    lower.set_ylim(lo_lo, lo_hi)
    lower.set_yticks(range(lo_lo + (-lo_lo) % 20, lo_hi + 1, 20))
    upper.spines["bottom"].set_visible(False)
    upper.tick_params(axis="x", labelbottom=False)
    for a, y0 in ((upper, 0), (lower, 1)):
        a.set_xlim(-0.35, len(LEVELS) - 0.35)
        h = top_h if a is upper else bot_h
        dy = 0.06 / h  # break marks the same physical size on both bands
        a.plot(
            [-0.012, 0.012],
            [y0 - dy, y0 + dy],
            transform=a.transAxes,
            color=INK,
            lw=1,
            clip_on=False,
        )
    # End labels: at least ~10% of the frontier band between neighbours so close finishes stay legible.
    gap = 0.10 * (up_hi - up_lo)
    label_y = {}
    end = {m: groups[m, "max"]["combined"]["mean"] for m in FRONTIER}
    for m in sorted(FRONTIER, key=end.get):
        label_y[m] = max(end[m], max(label_y.values(), default=-math.inf) + gap)
    label_y["beam"] = groups["beam", "max"]["combined"]["mean"]
    # Opus 5.5 and Sol nearly coincide at Low; a small sideways dodge keeps both whiskers visible.
    dodge = {"beam": 0.0, "opus55": -0.07, "gpt61sol": 0.0, "astra": 0.07}
    for m in MODELS:
        ax = lower if m == "beam" else upper
        xs = [LEVELS.index(e) + dodge[m] for e in LEVELS]
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
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
        ax.annotate(
            NAMES[m].replace("Claude ", "").replace("Reflection ", ""),
            (xs[-1], ys[-1]),
            xytext=(4.18, label_y[m]),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
        )
    for e in LEVELS:
        z = groups["beam", e]["zero"]
        if not z:
            continue
        x = LEVELS.index(e)
        lower.scatter(
            [x],
            [z["mean"]],
            s=64,
            marker=MARKERS["beam"],
            facecolors=PAPER,
            edgecolors=COLORS["beam"],
            linewidths=1.6,
            zorder=4,
        )
        lower.annotate(
            UNJUDGED_AS_ZERO["beam"],
            (x, z["mean"]),
            xytext=(8, -3),
            textcoords="offset points",
            fontsize=9,
            color=MUTED,
            fontfamily="Geist",
        )

    text(0.56, 3.0, "Time per task", 20, True, heading=True)
    text(0.56, 3.32, "minutes  ·  lower is faster", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, top_h + gap_h + bot_h)
    shifts = {"beam": -0.3, "opus55": -0.1, "gpt61sol": 0.1, "astra": 0.3}
    top = max(groups[m, e]["minutes"]["mean"] for m in MODELS for e in levels_of(m))
    ax.set_ylim(0, top * 1.22)
    for m in MODELS:
        idx = [LEVELS.index(e) for e in levels_of(m)]
        ys = [groups[m, e]["minutes"]["mean"] for e in levels_of(m)]
        ax.bar(
            [i + shifts[m] for i in idx],
            ys,
            width=0.2,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in zip(idx, ys, strict=True):
            ax.annotate(
                f"{y:.0f}" if y >= 10 else f"{y:.1f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=7.5,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    short = {m: NAMES[m].replace("Claude ", "").replace("Reflection ", "") for m in MODELS}
    partial = "; ".join(
        short[m]
        + " "
        + ", ".join(
            f"{e.replace('-', ' ').capitalize()} {groups[m, e]['combined']['n']}"
            for e in LEVELS
            if groups[m, e]["combined"]["n"] < 23
        )
        for m in MODELS
        if any(groups[m, e]["combined"]["n"] < 23 for e in LEVELS)
    )
    text(
        left,
        8.45,
        f"Judged runs: 23 per point except {partial}. Code quality judges: Muse Spark 1.3 with GPT-6.1 Sol "
        "(Beam) or Grok 4.6 (others). Harnesses differ.",
        11,
        color=MUTED,
    )

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"beam-vs-frontier-card{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"beam-vs-frontier-card{suffix}-efforts.csv"
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
                "opus48_reply_share_pct",
                "fallback_runs",
            ]
        )
        for (m, e), g in groups.items():
            w.writerow(
                [
                    m,
                    e,
                    g["n"],
                    g["combined"]["n"],
                    f"{g['combined']['mean']:.4f}",
                    f"{g['combined']['se']:.4f}",
                    "" if g["code_quality"]["mean"] is None else f"{g['code_quality']['mean']:.4f}",
                    g["passed"],
                    "" if g["usd"]["mean"] is None else f"{g['usd']['mean']:.4f}",
                    f"{g['minutes']['mean']:.4f}",
                    "" if g["fb_share"] is None else f"{g['fb_share']:.2f}",
                    g["fb_runs"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "final": final,
            "sources": sources,
            "best": best,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

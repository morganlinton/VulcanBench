"""Reflection Beam, Muse Wonder Full, Claude Opus 5.5 and GPT-6.1 Sol on VulcanBench Frontier v4.

One card in the format of the Sonnet 5.5 and DeepSeek V4.1 Flash comparison
cards: combined score and time per task at every effort level, and a table per
column. Reflection publishes no Beam price, so the right panel shows minutes,
not cost, and Beam's cost row reads n/a.

Sources, nothing re-run or re-judged:

- Claude Opus 5.5 and GPT-6.1 Sol: their published per-level rows (Code quality
  v3.15 and v3.18, Muse Spark 1.3 and Grok 4.6), as on the earlier cards.
- Muse Wonder Full: its published per-level rows (Code quality v3.26 to v3.26.3,
  Claude Opus 5.5 and GPT-6.1 Sol) from musewonder-full-vs-frontier.json, Low
  to Extra high; Max was not run in Contributor mode. Its cost is the published
  estimate at the Muse Wonder 1.0 list price.
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
import textwrap
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
WONDER = ROOT / "docs/results/swe-v4-musewonder-full-2026-10/musewonder-full-vs-frontier.json"
OUTPUT = ROOT / "docs/results/swe-v4-beam-2026-10"
RUNS = ROOT / "runs-effort-beam-mirror"
MODELS = ("beam", "wonder", "opus55", "gpt61sol")
NAMES = {
    "beam": "Reflection Beam",
    "wonder": "Muse Wonder Full",
    "opus55": "Claude Opus 5.5",
    "gpt61sol": "GPT-6.1 Sol",
}
# Muse Wonder Full keeps Meta blue; Beam takes a purple no lab on the board uses
# (the earlier violet was too close to Meta blue for deutan vision). Opus 5.5 and
# GPT-6.1 Sol keep their hues from the Sonnet 5.5 comparison card. All four pass
# the palette validator's all-pairs color-vision checks; markers, direct labels
# and the table carry identity as well as color.
COLORS = {"beam": "#9C27B0", "wonder": "#0866FF", "opus55": "#8C3A1F", "gpt61sol": "#10A37F"}
MARKERS = {"beam": "o", "wonder": "s", "opus55": "D", "gpt61sol": "^"}
UNJUDGED_AS_ZERO = {"beam": "no-code runs as 0", "wonder": "timeouts as 0"}
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
    """Per-level aggregates: Opus 5.5, GPT-6.1 Sol and Muse Wonder Full published, Beam from v3.28."""
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
    wonder = json.loads(WONDER.read_text())
    require(wonder["final"], "the Muse Wonder Full card table is not final")
    for e, w in wonder["groups"]["wonder"].items():
        groups["wonder", e] = {
            "n": w["n"],
            "combined": {"n": w["n_judged"], "mean": w["combined"], "se": w["combined_se"]},
            "zero": {"mean": w["combined_timeouts_zero"], "runs": w["timeouts"]}
            if w["timeouts"]
            else None,
            "code_quality": {"mean": None},
            "passed": w["passed"],
            "usd": {"mean": w["usd"], "estimate": True},
            "minutes": {"mean": w["minutes"]},
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
            p.name: digest(p.read_bytes()) for p in (PUBLISHED, SOL, SOL_COST, WONDER)
        },
        "v3.28_summary_sha256": digest((judging / "summary.json").read_bytes()),
    }
    return groups, True, sources


def levels_of(m):
    """Muse Wonder Full has no Max: Muse Code's Contributor mode does not offer it."""
    return LEVELS[:-1] if m == "wonder" else LEVELS


def main():  # noqa: PLR0912, PLR0915, one linear figure
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True)
    groups, final, sources = load_groups(parser.parse_args().judging)

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 19.3
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
    text(right, 0.65, "October 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    best = {m: max(levels_of(m), key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(
        left,
        1.72,
        "VulcanBench Frontier v4: Reflection Beam against the frontier",
        30,
        True,
        heading=True,
    )
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])
    head = "  ·  ".join(
        f"{NAMES[m].replace('Claude ', '').replace('Reflection ', '')} "
        f"{groups[m, best[m]]['combined']['mean']:.2f} ({best[m].replace('-', ' ').capitalize()})"
        for m in order
    )
    text(left, 2.22, f"Best combined score: {head}.", 14.5)
    text(
        left,
        2.60,
        "23 hard legacy-reconstruction tasks per effort level. Code quality panels differ by model (see notes); "
        "Muse Wonder Full has no Max level.",
        13.5,
        color=MUTED,
    )

    x = left
    for m in MODELS:
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(3.05)],
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
        text(x + 0.016, 3.05, label, 13, True)
        x += {"beam": 0.19, "wonder": 0.20, "opus55": 0.27, "gpt61sol": 0}[m]

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

    text(left, 3.62, "Combined score", 20, True, heading=True)
    text(
        left,
        3.94,
        "/100  ·  higher is better  ·  focused scale  ·  whiskers +/-1 se",
        12,
        color=MUTED,
    )
    ax = axis(0.085, 0.40, 4.25, 3.2)
    vals = [groups[m, e]["combined"] for m in MODELS for e in levels_of(m)]
    zeros = [
        (m, LEVELS.index(e), groups[m, e]["zero"]["mean"])
        for m in UNJUDGED_AS_ZERO
        for e in levels_of(m)
        if groups[m, e]["zero"]
    ]
    lo = math.floor(min([v["mean"] - v["se"] for v in vals] + [z for _, _, z in zeros])) - 2
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 5
    # End labels: keep at least 5% of the axis between neighbours so close finishes stay legible.
    gap = 0.05 * (hi - lo)
    label_y = {}
    end = {m: groups[m, levels_of(m)[-1]]["combined"]["mean"] for m in MODELS}
    for m in sorted(MODELS, key=end.get):
        y_end = end[m]
        floor = max(label_y.values(), default=-math.inf) + gap
        label_y[m] = max(y_end, floor)
    ax.set_ylim(lo, hi)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    for m in MODELS:
        xs = [LEVELS.index(e) for e in levels_of(m)]
        ys = [groups[m, e]["combined"]["mean"] for e in levels_of(m)]
        es = [groups[m, e]["combined"]["se"] for e in levels_of(m)]
        ax.plot(
            xs,
            ys,
            color=COLORS[m],
            lw=2,
            zorder=2,
        )
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
            xytext=(4.12, label_y[m]),  # one label column, past Max, for every model
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
        )

    for m, x, z in zeros:
        ax.scatter(
            [x],
            [z],
            s=64,
            marker=MARKERS[m],
            facecolors=PAPER,
            edgecolors=COLORS[m],
            linewidths=1.6,
            zorder=4,
        )
        ax.annotate(
            UNJUDGED_AS_ZERO[m],
            (x, z),
            xytext=(8, -3) if m == "beam" else (-8, -10),
            ha="left" if m == "beam" else "right",
            textcoords="offset points",
            fontsize=9,
            color=MUTED,
            fontfamily="Geist",
        )

    text(0.56, 3.62, "Time per task", 20, True, heading=True)
    text(
        0.56,
        3.94,
        "mean wall-clock minutes per task  ·  lower is faster  ·  Beam has no published price",
        12,
        color=MUTED,
    )
    ax = axis(0.585, 0.355, 4.25, 3.2)
    shifts = {"beam": -0.3, "wonder": -0.1, "opus55": 0.1, "gpt61sol": 0.3}
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

    ty = 8.35
    text(left, ty, "Table 1  |  Every effort level", 14.5, heading=True)
    cols = dict(zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True))
    line(left, right, ty + 0.28, INK, 1.2)
    text(left, ty + 0.52, "Model and measure", 12.5, True)
    for e in LEVELS:
        text(cols[e], ty + 0.52, e.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, ty + 0.72, INK, 0.6)
    y = ty + 0.98
    for m in MODELS:
        text(left, y, NAMES[m], 13, True)
        fig.add_artist(
            plt.Line2D(
                [left - 0.012],
                [yf(y)],
                transform=fig.transFigure,
                marker=MARKERS[m],
                color=COLORS[m],
                markersize=7,
                markeredgecolor=INK,
                markeredgewidth=0.5,
                linestyle="none",
            )
        )
        measures = [
            (
                "Combined score",
                lambda g: f"{g['combined']['mean']:.2f}" + ("*" if g["combined"]["n"] < 23 else ""),
                True,
            ),
            ("Tasks passed of 23", lambda g: str(g["passed"]), False),
            (
                "Cost per task, USD",
                lambda g: (
                    "n/a"
                    if g["usd"]["mean"] is None
                    else ("~" if g["usd"].get("estimate") else "") + f"{g['usd']['mean']:.2f}"
                ),
                False,
            ),
        ]
        if m in UNJUDGED_AS_ZERO:
            measures.insert(
                1,
                (
                    f"Combined, {UNJUDGED_AS_ZERO[m]}",
                    lambda g: f"{g['zero']['mean']:.2f}" if g["zero"] else "same",
                    False,
                ),
            )
        if m == "beam":
            measures.append(("Minutes per task", lambda g: f"{g['minutes']['mean']:.1f}", False))
            measures.append(
                ("Tokens per task, millions", lambda g: f"{g['tokens']['mean'] / 1e6:.2f}", False)
            )
        if m in CLAUDE_MAIN:
            measures.append(
                (
                    "Fallback share of replies (runs)",
                    lambda g: f"{g['fb_share']:.1f}% ({g['fb_runs']})",
                    False,
                )
            )
        for label, fn, strong in measures:
            y += 0.30
            text(left + 0.012, y, label, 12, color=INK if strong else MUTED)
            for e in LEVELS:
                if (m, e) not in groups:
                    text(cols[e], y, "not run", 12, ha="right", color=MUTED)
                    continue
                g = groups[m, e]
                emph = strong and e == best[m]
                text(
                    cols[e],
                    y,
                    fn(g),
                    13 if strong else 12,
                    emph,
                    numeric=True,
                    ha="right",
                    color=INK,
                )
        y += 0.40
        line(left, right, y - 0.2, RULE, 0.6)
    line(left, right, y - 0.2, INK, 1.2)

    notes = [
        "* Combined score over the judged runs when fewer than 23. Bold marks each model's best level. Beam's Low and Medium include "
        "runs that changed no source code (12 at Low, 3 at Medium, file-mode or scratch changes only, or none); they are not judged "
        "and count as failed, and the row below shows the combined score counting them as 0. Max is over 22: one run's evidence was "
        "too large for either judge to review.",
        "Muse Wonder Full: High and Extra high are over 22 judged runs; paddockcore (High) and depotcore (Extra high) hit the 3-hour "
        "task limit and left no finished code, and the row below counts them as 0. It ran on a Muse Code subscription in Contributor "
        "mode, which offers no Max; Meta lists no API price, so its cost (~) is an estimate at the Muse Wonder 1.0 list price. Its rows "
        "are as published on 18 October; Extra high depotcore overlapped another local job for 145 of its 180 minutes, and its clean "
        "rerun has not been judged yet.",
        "Protocols: Beam is Code quality v3.28, Muse Wonder Full v3.26 to v3.26.3, Opus 5.5 v3.15 and GPT-6.1 Sol v3.18, with the "
        "same rubric, controls and weights. Opus 5.5 and Sol were judged by Muse Spark 1.3 and Grok 4.6; Beam by Muse Spark 1.3 and "
        "GPT-6.1 Sol, because Grok 4.6's Cursor account was out of usage; Muse Wonder Full by Claude Opus 5.5 and GPT-6.1 Sol, because "
        "Muse Spark is a Meta model. The panels are not calibrated against each other, and Opus 5.5 and Sol judged columns they are "
        "shown beside.",
        "Beam ran in Reflection's Mirror CLI 0.1.1 (early beta) through Reflection's API; Reflection publishes no price, so it has no "
        "cost. Mirror labels a Permission denied error as a sandbox denial, and two Low runs ended believing the sandbox blocked "
        "the legacy binary. A daily API token limit paused the sweep once; no finished run was affected.",
        "Harnesses differ (Mirror CLI 0.1.1, Muse Code 1.4.6 and 1.4.9, Claude Code 2.1.280, Codex 0.159.0), so small gaps are "
        "harness confounded. Muse Wonder Full, Opus 5.5 and GPT-6.1 Sol numbers are their published per-level rows; nothing was "
        "re-run or re-judged.",
    ]
    yy = y + 0.12
    for n in notes:
        for w in textwrap.wrap(n, 200):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

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

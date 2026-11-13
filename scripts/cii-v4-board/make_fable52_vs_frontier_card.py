"""Claude Fable 5.2, Claude Fable 5.1, Claude Opus 5.5 and GPT-6 Astra on VulcanBench Frontier v4.

The owner's comparison set for Fable 5.2 (chat, 2026-11-09): GPT-6 Astra, Opus
5.5 and Fable 5.1. A compact 16:9 card in the minimal layout (Beam and Terra
cards): the model and version as the title, two charts, combined score and cost
per task at every effort level, and one footnote line that keeps the judged run
counts, both judge panels and the cost basis visible. The per-level table,
minutes and Code quality included, is written to the CSV beside the card.

Sources, nothing re-run:

- Claude Fable 5.2: the Code quality v3.30.2 summary (Low to Extra high) and
  v3.30.3 summary (Max), Muse Spark 1.3 and GPT-6.1 Sol, and its run summaries
  in runs-effort-fable52. Cost is Claude Code's own list-price total
  (`cli_reported_cost_usd`), the basis of the other Claude columns; the
  adapter's own estimate folds cached prompt tokens in at the full input price.
- Claude Fable 5.1 and Claude Opus 5.5: Code quality and combined score from the
  matched-panel rounds v3.30.4 and v3.30.5 (owner request, 2026-11-12), where
  GPT-6.1 Sol reviewed the frozen v3.4 and v3.15 populations beside Muse Spark
  1.3's existing reviews, so all three Claude columns share one panel. Runs
  solved, cost and minutes stay their published per-level rows
  (docs/results/swe-v4-opus55-2026-09/opus55-astra-fable-efforts.csv), and the
  board keeps their published Muse and Grok 4.6 scores.
- GPT-6 Astra: its published row (v3.4, Muse Spark 1.3 and Grok 4.6). Sol cannot
  judge it (same lab), so it is footnoted; muse-like-for-like.csv compares all
  four models under Muse alone.

    python scripts/cii-v4-board/make_fable52_vs_frontier_card.py \\
        --judging <v3.30.2 directory> --judging-max <v3.30.3 directory> \\
        --matched-fable51 <v3.30.4 directory> --matched-opus55 <v3.30.5 directory>
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
OUTPUT = ROOT / "docs/results/swe-v4-fable52-2026-11"
RUNS = ROOT / "runs-effort-fable52"
SPEC = "claude-code:claude-fable-5-2"
CLI = "2.1.321 (Claude Code)"
PROTOCOLS = {
    "judging": "code-quality-maintenance-v3.30.2",
    "judging_max": "code-quality-maintenance-v3.30.3",
}
MODELS = ("fable52", "fable51", "opus55", "astra")
NAMES = {
    "fable52": "Claude Fable 5.2",
    "fable51": "Claude Fable 5.1",
    "opus55": "Claude Opus 5.5",
    "astra": "GPT-6 Astra",
}
PUBLISHED_ID = {"fable51": "fable", "opus55": "opus55", "astra": "astra"}
MATCHED = {
    "fable51": "code-quality-maintenance-v3.30.4",
    "opus55": "code-quality-maintenance-v3.30.5",
}
MUSE_ONLY = OUTPUT / "muse-like-for-like.csv"
JUDGES = {
    "fable52": "muse+sol",
    "fable51": "muse+sol",
    "opus55": "muse+sol",
    "astra": "muse+grok",
}
# Fable 5.2 takes Anthropic clay, Opus 5.5 keeps its dark clay from earlier cards,
# Fable 5.1 an indigo (its warm gray on the Sonnet card reads as gray here) and
# Astra OpenAI green. All four pass the all-pairs checks of the dataviz validator on
# the card paper; the worst pair (Astra and Fable 5.2, CVD 6.8) sits in the band that
# needs secondary encoding, which the markers and direct labels give.
COLORS = {"fable52": "#D97757", "fable51": "#5E5BA8", "opus55": "#8C3A1F", "astra": "#10A37F"}
MARKERS = {"fable52": "o", "fable51": "s", "opus55": "D", "astra": "^"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
WITH_FALLBACK = ("fable51", "opus55")


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def fable52_runs(effort):
    runs = [
        json.loads(p.read_text()) for p in sorted((RUNS / effort).glob("legacy-*/summary.json"))
    ]
    require(len(runs) == 23, f"Fable 5.2 {effort} is not 23 runs")
    for r in runs:
        require(r["model"] == SPEC, f"Fable 5.2 {effort} has another solver")
        require(r["cli_agent"]["harness_version"] == CLI, f"Fable 5.2 {effort} ran on another CLI")
        require(
            set(r["served_models"]) == {"claude-fable-5-2"},
            f"Fable 5.2 {effort} {r['task_id']} served another model",
        )
    return runs


def load_summary(directory: Path, protocol: str):
    summary = json.loads((directory / "summary.json").read_text())
    require(summary["protocol"] == protocol, f"{directory} is not {protocol}")
    require(summary["ready_for_publication"], f"{protocol} is not ready for publication")
    require(
        summary["failed_panels"] == [] and set(summary["passing_panels"]) == {"muse", "sol"},
        f"{protocol} does not publish from Muse Spark 1.3 and GPT-6.1 Sol",
    )
    return summary


def apply_matched(groups, matched: dict[str, Path]):
    """Fable 5.1 and Opus 5.5 Code quality and combined score from their Muse and Sol rounds.

    Each round must hold the published population (same judged count per level),
    and its Muse half must equal Muse's own scores in muse-like-for-like.csv,
    which rebuilt them from the v3.4 and v3.15 directories.
    """
    with MUSE_ONLY.open() as handle:
        muse = {(r["model"], r["effort"]): r for r in csv.DictReader(handle)}
    digests = {}
    for m, directory in matched.items():
        summary = load_summary(directory, MATCHED[m])
        require(
            summary["published_submissions"] == summary["expected_submissions"],
            f"{MATCHED[m]} did not publish every submission",
        )
        require(summary["fallback_reviews"] == {"muse": 0, "sol": 0}, f"{MATCHED[m]} fell back")
        for e in LEVELS:
            g = summary["groups"][f"{PUBLISHED_ID[m]}/{e}"]
            require(
                g["composite_v3"]["n"] == groups[m, e]["combined"]["n"],
                f"{MATCHED[m]} {e} judged another population",
            )
            require(
                abs(g["by_panel"]["muse"]["mean"] - float(muse[m, e]["muse_review"])) < 1e-3,
                f"{MATCHED[m]} {e} Muse half differs from the Muse-only table",
            )
            groups[m, e]["combined"] = g["composite_v3"]
            groups[m, e]["code_quality"] = g["code_quality"]
        digests[f"{MATCHED[m].removeprefix('code-quality-maintenance-')}_summary_sha256"] = digest(
            (directory / "summary.json").read_bytes()
        )
    return digests


def load_groups(judging: Path, judging_max: Path, matched: dict[str, Path]):
    """Per-level aggregates: Fable 5.2 from v3.30.2 and v3.30.3, Fable 5.1 and Opus 5.5
    with Muse and Sol from v3.30.4 and v3.30.5, Astra as published."""
    groups = {}
    with PUBLISHED.open() as handle:
        rows = {(r["model"], r["effort"]): r for r in csv.DictReader(handle)}
    for m, pid in PUBLISHED_ID.items():
        for e in LEVELS:
            r = rows[pid, e]
            groups[m, e] = {
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
                "fb_share": float(r["opus48_reply_share_pct"])
                if r["opus48_reply_share_pct"]
                else None,
            }
    summaries = {
        "judging": load_summary(judging, PROTOCOLS["judging"]),
        "judging_max": load_summary(judging_max, PROTOCOLS["judging_max"]),
    }
    for e in LEVELS:
        summary = summaries["judging_max" if e == "max" else "judging"]
        g = summary["groups"][f"fable52/{e}"]
        published = [
            r["published"]["composite_v3"]
            for r in summary["rows"]
            if r["model"] == "fable52" and r["effort"] == e and r.get("published")
        ]
        require(len(published) == g["composite_v3"]["n"] == 23, f"Fable 5.2 {e} is not 23 judged")
        runs = fable52_runs(e)
        groups["fable52", e] = {
            "n": 23,
            "combined": g["composite_v3"],
            "code_quality": g["code_quality"],
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "usd": {"mean": statistics.mean(r["economics"]["cli_reported_cost_usd"] for r in runs)},
            "minutes": {"mean": statistics.mean(r["duration_s"] for r in runs) / 60},
            "fb_runs": 0,
            "fb_share": 0.0,
        }
    for m in MODELS:
        for e in LEVELS:
            require((m, e) in groups, f"{m} {e} missing")
    matched_digests = apply_matched(groups, matched)
    sources = {
        "published_table": {PUBLISHED.name: digest(PUBLISHED.read_bytes())},
        "muse_only_table": {MUSE_ONLY.name: digest(MUSE_ONLY.read_bytes())},
        "v3.30.2_summary_sha256": digest((judging / "summary.json").read_bytes()),
        "v3.30.3_summary_sha256": digest((judging_max / "summary.json").read_bytes()),
        **matched_digests,
    }
    return groups, sources


def main():  # noqa: PLR0915, one linear figure
    parser = argparse.ArgumentParser()
    parser.add_argument("--judging", type=Path, required=True, help="the v3.30.2 directory")
    parser.add_argument("--judging-max", type=Path, required=True, help="the v3.30.3 directory")
    parser.add_argument("--matched-fable51", type=Path, required=True, help="the v3.30.4 directory")
    parser.add_argument("--matched-opus55", type=Path, required=True, help="the v3.30.5 directory")
    args = parser.parse_args()
    groups, sources = load_groups(
        args.judging,
        args.judging_max,
        {"fable51": args.matched_fable51, "opus55": args.matched_opus55},
    )

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

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "Claude Fable 5.2", 30, True, heading=True)
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])

    x = left
    widths = {"fable52": 0.15, "fable51": 0.235, "opus55": 0.235, "astra": 0}
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
        label = f"{NAMES[m]}{' (with fallback)' if m in WITH_FALLBACK else ''}"
        text(x + 0.016, 2.4, label, 13, True)
        x += widths[m]

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
    text(left, 3.32, "higher is better  ·  whiskers ±1 se", 12, color=MUTED)
    plot_h = 3.9
    every = [groups[m, e]["combined"] for m in MODELS for e in LEVELS]
    lo = math.floor(min(v["mean"] - v["se"] for v in every)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in every)) + 1
    score = axis(0.085, 0.40, 3.7, plot_h)
    score.set_xlim(-0.35, len(LEVELS) - 0.35)
    score.set_ylim(lo, hi)
    step = 2 if hi - lo <= 16 else 5
    score.set_yticks(range(lo + (-lo) % step, hi + 1, step))
    # End labels at least 6% of the scale apart, then re-centred on the line ends.
    end = {m: groups[m, "max"]["combined"]["mean"] for m in MODELS}
    gap = 0.06 * (hi - lo)
    placed = {}
    for m in sorted(MODELS, key=end.get):
        placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
    shift = statistics.mean(placed.values()) - statistics.mean(end.values())
    label_y = {m: y - shift for m, y in placed.items()}
    dodge = {"fable52": -0.09, "fable51": -0.03, "opus55": 0.03, "astra": 0.09}
    for m in MODELS:
        xs = [LEVELS.index(e) + dodge[m] for e in LEVELS]
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        score.plot(xs, ys, color=COLORS[m], lw=2, zorder=2)
        score.errorbar(
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
        score.annotate(
            NAMES[m].replace("Claude ", ""),
            (xs[-1], ys[-1]),
            xytext=(4.18, label_y[m]),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
            annotation_clip=False,
        )

    text(0.56, 3.0, "Cost per task", 20, True, heading=True)
    text(0.56, 3.32, "US dollars at list price  ·  lower is cheaper", 12, color=MUTED)
    ax = axis(0.585, 0.355, 3.7, plot_h)
    # Bars 0.22 apart and 0.2 wide leave a gap between neighbours; at 6 points the
    # four-character dollar labels of neighbouring bars (Low, Max) no longer touch.
    shifts = {"fable52": -0.33, "fable51": -0.11, "opus55": 0.11, "astra": 0.33}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(len(LEVELS))],
            ys,
            width=0.2,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
    # A label whose left neighbour ends at nearly the same height is lifted one line,
    # so neighbouring dollar labels (Low, Max) never overlap.
    for i, e in enumerate(LEVELS):
        previous, lifted = None, False
        for m in MODELS:
            y = groups[m, e]["usd"]["mean"]
            lifted = previous is not None and abs(y - previous) < 0.05 * top and not lifted
            previous = y
            ax.annotate(
                f"{y:.2f}" if y < 10 else f"{y:.1f}",
                (i + shifts[m], y),
                xytext=(0, 11 if lifted else 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=6.0,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    short = {m: NAMES[m].replace("Claude ", "") for m in MODELS}
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
    judged = f"23 per point except {partial}" if partial else "23 per point"
    text(
        left,
        8.3,
        f"Judged runs: {judged}. Code quality judges: Muse Spark 1.3 and GPT-6.1 Sol, "
        "except Astra (Muse and Grok 4.6; Sol shares its lab).",
        11,
        color=MUTED,
    )
    text(
        left,
        8.6,
        "Cost is API-equivalent at list price (Claude: Claude Code's own total; Astra estimated). "
        "Harnesses differ.",
        11,
        color=MUTED,
    )

    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / "fable52-vs-frontier.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "fable52-vs-frontier-efforts.csv"
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
                "judges",
            ]
        )
        for m in MODELS:
            for e in LEVELS:
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
                        "" if g["fb_share"] is None else f"{g['fb_share']:.2f}",
                        g["fb_runs"],
                        JUDGES[m],
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
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

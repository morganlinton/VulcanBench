"""GPT-6.1 Terra, Claude Opus 5.5, GPT-6.1 Sol and GPT-6 Astra on VulcanBench Frontier v4.

A compact 16:9 card in the Beam card's layout (owner request 2026-10-28): the
model and version as the title, then two charts, combined score and cost per
task at every effort level, and one footnote line that keeps the judged run
counts, the judge panel and the cost basis visible. All four models are
priced, so the right panel shows cost (Beam's showed minutes because Beam has
no price). The per-level table, minutes included, is written to the CSV beside
the card.

The combined score panel breaks its axis only when Terra's whole curve sits
below the frontier band, as Beam's did; when the curves overlap, one shared
scale is used and the end labels are spaced apart.

Sources, nothing re-run or re-judged:

- Claude Opus 5.5, GPT-6.1 Sol and GPT-6 Astra: their published per-level rows
  (Code quality v3.15, v3.18 and v3.4, all Muse Spark 1.3 and Grok 4.6), as on
  the Beam card. Astra's cost is its report's central estimate at list price.
- GPT-6.1 Terra: the Code quality v3.29 summary (Muse Spark 1.3 and Grok 4.6)
  and its run summaries in runs-effort-gpt61-terra. Cost is the API-equivalent
  at OpenAI's list price from the tokens Codex reported (subscription runs).

    python scripts/cii-v4-board/make_gpt61terra_vs_frontier_card.py --judging <v3.29 run directory>
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
OUTPUT = ROOT / "docs/results/swe-v4-gpt61-terra-2026-10"
RUNS = ROOT / "runs-effort-gpt61-terra"
SPEC = "codex:gpt-6.1-terra"
MODELS = ("gpt61terra", "opus55", "gpt61sol", "astra")
FRONTIER = ("opus55", "gpt61sol", "astra")
NAMES = {
    "gpt61terra": "GPT-6.1 Terra",
    "opus55": "Claude Opus 5.5",
    "gpt61sol": "GPT-6.1 Sol",
    "astra": "GPT-6 Astra",
}
# Opus 5.5, GPT-6.1 Sol and GPT-6 Astra keep their hues from the Beam card. Terra takes
# a deep blue no column on this card uses; all four pass the all-pairs color-vision
# checks on the card paper (worst pair Sol and Astra, as on the Beam card), and
# markers, direct labels and value labels carry identity as well as color.
COLORS = {"gpt61terra": "#2F5DA8", "opus55": "#8C3A1F", "gpt61sol": "#10A37F", "astra": "#B8860B"}
MARKERS = {"gpt61terra": "o", "opus55": "D", "gpt61sol": "^", "astra": "s"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CLAUDE_MAIN = {"opus55": "claude-opus-5-5"}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def load_groups(judging: Path):
    """Per-level aggregates: Opus 5.5, GPT-6.1 Sol and GPT-6 Astra published, Terra from v3.29."""
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
        "a v3.29 panel failed",
    )
    for e in LEVELS:
        runs = [json.loads(p.read_text()) for p in sorted((RUNS / e).glob("legacy-*/summary.json"))]
        require(len(runs) == 23, f"Terra {e} is not 23 runs")
        require(all(r["model"] == SPEC for r in runs), f"Terra {e} has another solver")
        g = summary["groups"][f"gpt61terra/{e}"]
        published = [
            r["published"]["composite_v3"]
            for r in summary["rows"]
            if r["model"] == "gpt61terra" and r["effort"] == e and r.get("published")
        ]
        require(
            len(published) == g["composite_v3"]["n"],
            f"Terra {e} summary rows disagree with its group",
        )
        groups["gpt61terra", e] = {
            "n": 23,
            "combined": g["composite_v3"],
            "code_quality": g["code_quality"],
            "passed": sum(r["scores"]["functional"] >= 1 for r in runs),
            "usd": {
                "mean": statistics.mean(r["economics"]["api_equivalent_cost_usd"] for r in runs)
            },
            "minutes": {"mean": statistics.mean(r["duration_s"] for r in runs) / 60},
            "fb_runs": 0,
            "fb_share": None,
        }
    for m in MODELS:
        for e in LEVELS:
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "published_tables": {
            p.name: digest(p.read_bytes()) for p in (PUBLISHED, SOL, SOL_COST, ASTRA, ASTRA_COST)
        },
        "v3.29_summary_sha256": digest((judging / "summary.json").read_bytes()),
    }
    return groups, True, sources


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

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(left, 1.72, "GPT-6.1 Terra", 30, True, heading=True)
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
        x += {"gpt61terra": 0.17, "opus55": 0.27, "gpt61sol": 0.15, "astra": 0}[m]

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
    top_h, gap_h, bot_h = 2.1, 0.14, 1.66
    front = [groups[m, e]["combined"] for m in FRONTIER for e in LEVELS]
    terra = [groups["gpt61terra", e]["combined"] for e in LEVELS]
    up_lo = math.floor(min(v["mean"] - v["se"] for v in front))
    up_hi = math.ceil(max(v["mean"] + v["se"] for v in front)) + 1
    terra_hi = max(v["mean"] + v["se"] for v in terra)
    broken = terra_hi + 3 < up_lo
    if broken:
        # Broken axis, as on the Beam card: a zoomed frontier band over Terra's own band.
        upper = axis(0.085, 0.40, 3.7, top_h)
        lower = axis(0.085, 0.40, 3.7 + top_h + gap_h, bot_h)
        lo_lo = 5 * math.floor((min(v["mean"] - v["se"] for v in terra) - 3) / 5)
        lo_hi = 5 * math.ceil((terra_hi + 2) / 5)
        require(
            lo_hi < up_lo, "Terra and the frontier band overlap; the broken axis would hide data"
        )
        note = f"higher is better  ·  axis broken between {lo_hi} and {up_lo}  ·  whiskers ±1 se"
        upper.set_ylim(up_lo, up_hi)
        upper.set_yticks(range(up_lo + up_lo % 2, up_hi + 1, 2))
        lower.set_ylim(lo_lo, lo_hi)
        lower.set_yticks(range(lo_lo + (-lo_lo) % 10, lo_hi + 1, 10))
        upper.spines["bottom"].set_visible(False)
        upper.tick_params(axis="x", labelbottom=False)
        for a, y0 in ((upper, 0), (lower, 1)):
            a.set_xlim(-0.35, len(LEVELS) - 0.35)
            dy = 0.06 / (top_h if a is upper else bot_h)
            a.plot(
                [-0.012, 0.012],
                [y0 - dy, y0 + dy],
                transform=a.transAxes,
                color=INK,
                lw=1,
                clip_on=False,
            )
        home = {m: upper for m in FRONTIER} | {"gpt61terra": lower}
        spans = {upper: up_hi - up_lo, lower: lo_hi - lo_lo}
    else:
        # Terra overlaps the frontier band, so one shared scale; no break.
        both = front + terra
        lo = math.floor(min(v["mean"] - v["se"] for v in both)) - 1
        hi = math.ceil(max(v["mean"] + v["se"] for v in both)) + 1
        single = axis(0.085, 0.40, 3.7, top_h + gap_h + bot_h)
        single.set_xlim(-0.35, len(LEVELS) - 0.35)
        single.set_ylim(lo, hi)
        step = 2 if hi - lo <= 16 else 5
        single.set_yticks(range(lo + (-lo) % step, hi + 1, step))
        note = "higher is better  ·  whiskers ±1 se"
        home = dict.fromkeys(MODELS, single)
        spans = {single: hi - lo}
    text(left, 3.32, note, 12, color=MUTED)
    # End labels: at least ~6% of their band apart so close finishes stay legible, then the
    # stack is re-centred on the line ends so no label drifts far from its line.
    label_y = {}
    for ax_ in set(home.values()):
        members = [m for m in MODELS if home[m] is ax_]
        end = {m: groups[m, "max"]["combined"]["mean"] for m in members}
        gap = 0.06 * spans[ax_]
        placed = {}
        for m in sorted(members, key=end.get):
            placed[m] = max(end[m], max(placed.values(), default=-math.inf) + gap)
        shift = statistics.mean(placed.values()) - statistics.mean(end.values())
        label_y |= {m: y - shift for m, y in placed.items()}
    dodge = {"gpt61terra": -0.03, "opus55": -0.09, "gpt61sol": 0.03, "astra": 0.09}
    for m in MODELS:
        ax = home[m]
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
    ax = axis(0.585, 0.355, 3.7, top_h + gap_h + bot_h)
    # Four-character dollar labels are wider than a 0.2 bar, so bars sit 0.22 apart and the
    # labels are a size smaller than on the Beam card; neighbours' labels then never touch.
    shifts = {"gpt61terra": -0.33, "opus55": -0.11, "gpt61sol": 0.11, "astra": 0.33}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    ax.yaxis.set_major_formatter(lambda v, _pos: f"${v:.0f}")
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(len(LEVELS))],
            ys,
            width=0.21,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in enumerate(ys):
            ax.annotate(
                f"{y:.2f}" if y < 10 else f"{y:.1f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=6.6,
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
        8.45,
        f"Judged runs: {judged}. Code quality judges: Muse Spark 1.3 and Grok 4.6. "
        "Cost is API-equivalent at list price (Astra estimated). Harnesses differ.",
        11,
        color=MUTED,
    )

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"terra-vs-frontier-card{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"terra-vs-frontier-card{suffix}-efforts.csv"
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
            "broken_axis": broken,
            "png_sha256": digest(out.read_bytes()),
            "table": table.name,
        },
    )
    print(out)
    for m in order:
        print(f"  {NAMES[m]:<18} best {best[m]:<10} {groups[m, best[m]]['combined']['mean']:.2f}")


if __name__ == "__main__":
    main()

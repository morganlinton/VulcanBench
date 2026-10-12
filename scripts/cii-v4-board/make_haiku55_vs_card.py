"""Claude Haiku 5.5 beside one other Frontier v4 model: Claude Sonnet 5.5 or GPT-6.1 Sol.

One card per comparison, in the layout of the Haiku 5.5 vs GPT-6 Luna card:
combined score and cost per task at every effort level, and a table with tasks
passed, cost and the refusal-fallback share.

Sources, nothing re-judged:

- Claude Sonnet 5.5 (Code quality v3.23) and GPT-6.1 Sol (v3.18): their
  published per-level rows in
  docs/results/swe-v4-sonnet55-2026-10/sonnet55-vs-frontier-efforts.csv.
- Claude Haiku 5.5: the Code quality v3.24 summary and manifest, aggregated by
  make_haiku55_v324_card.py (cost is Claude Code's own list-price total).

    python scripts/cii-v4-board/make_haiku55_vs_card.py --vs sonnet55
    python scripts/cii-v4-board/make_haiku55_vs_card.py --vs gpt61sol
"""

from __future__ import annotations

import csv
import importlib.util
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

PUBLISHED = ROOT / "docs/results/swe-v4-sonnet55-2026-10/sonnet55-vs-frontier-efforts.csv"
OUTPUT = ROOT / "docs/results/swe-v4-haiku55-2026-10"
MODELS = ("haiku55", "other")
NAMES = {"haiku55": "Claude Haiku 5.5", "other": ""}
HARNESS = {
    "haiku55": "Claude Code 2.1.293 to 2.1.296",
    "other": "",
}
# Lab colors (CLAUDE.md): Anthropic clay and OpenAI green. Markers, direct
# labels and the table carry identity as well as color.
COLORS = {"haiku55": "#D97757", "other": ""}
MARKERS = {"haiku55": "o", "other": "D"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
CLAUDE_MAIN = {"haiku55": "claude-haiku-5-5"}


def _haiku_card():
    spec = importlib.util.spec_from_file_location(
        "haiku55_card", Path(__file__).with_name("make_haiku55_v324_card.py")
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


OPPONENTS = {
    "sonnet55": {
        "name": "Claude Sonnet 5.5",
        "harness": "Claude Code 2.1.291 to 2.1.293",
        "color": "#8C3A1F",
        "protocol": "v3.23",
        "note": "Both ran through Claude Code with the refusal fallback on, and neither used it; costs are Claude "
        "Code's own list-price totals (Haiku 5.5 sits between the floor and ceiling of its two price tiers, shown on "
        "its own card). Claude Code versions differ (2.1.293 to 2.1.296 against 2.1.291 to 2.1.293), and both rounds' "
        "Grok calls ran on Cursor CLI 2026.10.01.",
    },
    "gpt61sol": {
        "name": "GPT-6.1 Sol",
        "harness": "Codex 0.159.0",
        "color": "#10A37F",
        "protocol": "v3.18",
        "note": "Cost bases differ: Haiku 5.5 is Claude Code's own list-price total, between the floor and ceiling of "
        "its two price tiers shown on its own card; GPT-6.1 Sol is API-equivalent at list prices from its token "
        "ledger, and its runs used the ChatGPT Pro subscription. Codex has no refusal fallback. Harnesses differ "
        "(Claude Code against Codex 0.159.0), so small gaps are harness confounded.",
    },
}
PROTOCOL = {k: v["protocol"] for k, v in OPPONENTS.items()}
NOTES = {k: v["note"] for k, v in OPPONENTS.items()}
OTHER = "sonnet55"


def select(other: str) -> None:
    """Point the card at one opponent."""
    global OTHER  # noqa: PLW0603, one card per run
    OTHER = other
    o = OPPONENTS[other]
    NAMES["other"], HARNESS["other"], COLORS["other"] = o["name"], o["harness"], o["color"]
    if other == "sonnet55":
        CLAUDE_MAIN["other"] = "claude-sonnet-5-5"
    else:
        CLAUDE_MAIN.pop("other", None)


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = [v for v in values if v is not None]
    if not values:
        return {"n": 0, "mean": None, "se": None}
    se = statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0
    return {"n": len(values), "mean": statistics.mean(values), "se": se}


def load_groups():
    """Per-level aggregates: the other model from its published table, Haiku 5.5 from v3.24."""
    groups = {}
    with PUBLISHED.open() as handle:
        for r in csv.DictReader(handle):
            if r["model"] != OTHER:
                continue
            groups["other", r["effort"]] = {
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
                "fb_runs": int(r["fallback_runs"] or 0),
                "fb_share": 0.0 if "other" in CLAUDE_MAIN else None,
            }
    global HAIKU_UNJUDGED  # noqa: PLW0603, filled from the data before the figure
    card = HAIKU
    _summary, _protocol, rows, final, coverage = card.load()
    excluded = json.loads(card.POPULATION.read_text())["excluded"]
    cells = ", ".join(
        f"{e['effort'].replace('-', ' ')} {e['task'].removeprefix('legacy-').split('-')[0]}"
        for e in excluded
    )
    invalid = ", ".join(
        f"{i['effort'].replace('-', ' ')} {i['task'].removeprefix('legacy-').split('-')[0]}"
        for i in coverage["unpublished_invalid_probe"]
    )
    HAIKU_UNJUDGED = (
        f"Unjudged Haiku runs: {cells} (a safety classifier stopped the session before any edit; counted as failed)"
        + (
            f", and {invalid} (no valid judge probe, so unpublished under the frozen protocol)."
            if invalid
            else "."
        )
    )
    card.ALL_RUNS = card.all_runs()
    for (_m, e), g in card.aggregate(rows).items():
        groups["haiku55", e] = {
            "n": 23,
            "combined": g["combined"],
            "code_quality": g["code_quality"],
            "passed": card.ALL_RUNS[e]["passed"],
            "usd": g["cli_usd"],
            "minutes": g["minutes"],
            "fb_runs": g["fallbacks"],
            "fb_share": 100
            * sum(
                sum(
                    v
                    for k, v in r["replies"].items()
                    if k not in (CLAUDE_MAIN["haiku55"], "<synthetic>")
                )
                for r in rows
                if r["effort"] == e
            )
            / max(1, sum(sum(r["replies"].values()) for r in rows if r["effort"] == e)),
        }
    for m in MODELS:
        for e in LEVELS:
            require((m, e) in groups, f"{m} {e} missing")
    sources = {
        "published_table": {PUBLISHED.name: digest(PUBLISHED.read_bytes())},
        "v3.24_summary_sha256": digest((card.RUN / "summary.json").read_bytes()),
    }
    return groups, final, sources


HAIKU = _haiku_card()
HAIKU_UNJUDGED = ""


def main():  # noqa: PLR0912, PLR0915, one linear figure
    groups, final, sources = load_groups()

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    W, H = 16, 15.6
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

    best = {m: max(LEVELS, key=lambda e: groups[m, e]["combined"]["mean"]) for m in MODELS}
    text(
        left,
        1.72,
        f"VulcanBench Frontier v4: Claude Haiku 5.5 and {NAMES['other']}",
        30,
        True,
        heading=True,
    )
    order = sorted(MODELS, key=lambda m: -groups[m, best[m]]["combined"]["mean"])
    head = "  ·  ".join(
        f"{NAMES[m]} {groups[m, best[m]]['combined']['mean']:.2f} at {best[m].replace('-', ' ')}"
        for m in order
    )
    text(left, 2.22, f"Best combined score per model: {head}.", 15)
    text(
        left,
        2.60,
        "23 hard legacy-reconstruction tasks per effort level. Code quality judged by Muse Spark 1.3 and Grok 4.6.",
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
        x += {"haiku55": 0.30, "other": 0}[m]

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
    vals = [groups[m, e]["combined"] for m in MODELS for e in LEVELS]
    lo = math.floor(min(v["mean"] - v["se"] for v in vals)) - 1
    hi = math.ceil(max(v["mean"] + v["se"] for v in vals)) + 1
    # End labels: keep at least ~6.5% of the axis between neighbours so close finishes stay legible.
    gap = 0.065 * (hi - lo)
    label_y = {}
    for m in sorted(MODELS, key=lambda m: groups[m, "max"]["combined"]["mean"]):
        y_end = groups[m, "max"]["combined"]["mean"]
        floor = max(label_y.values(), default=-math.inf) + gap
        label_y[m] = max(y_end, floor)
    ax.set_ylim(lo, hi)
    ax.set_xlim(-0.35, len(LEVELS) - 0.35)
    for m in MODELS:
        ys = [groups[m, e]["combined"]["mean"] for e in LEVELS]
        es = [groups[m, e]["combined"]["se"] for e in LEVELS]
        ax.plot(range(5), ys, color=COLORS[m], lw=2, zorder=2)
        ax.errorbar(
            range(5),
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
            (4, ys[-1]),
            xytext=(4.12, label_y[m]),
            textcoords="data",
            va="center",
            fontsize=10.5,
            color=INK,
            fontfamily="Geist",
            weight=700,
        )

    text(0.56, 3.62, "Cost per task", 20, True, heading=True)
    text(
        0.56,
        3.94,
        "USD, API-equivalent at list prices  ·  every serving model included  ·  bases in notes",
        12,
        color=MUTED,
    )
    ax = axis(0.585, 0.355, 4.25, 3.2)
    shifts = {"haiku55": -0.17, "other": 0.17}
    top = max(groups[m, e]["usd"]["mean"] for m in MODELS for e in LEVELS)
    ax.set_ylim(0, top * 1.22)
    for m in MODELS:
        ys = [groups[m, e]["usd"]["mean"] for e in LEVELS]
        ax.bar(
            [i + shifts[m] for i in range(5)],
            ys,
            width=0.25,
            color=COLORS[m],
            edgecolor=INK,
            linewidth=0.5,
            zorder=2,
        )
        for i, y in enumerate(ys):
            ax.annotate(
                f"{y:.2f}",
                (i + shifts[m], y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8.5,
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
            ("Cost per task, USD", lambda g: f"{g['usd']['mean']:.2f}", False),
        ]
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
        "* Combined score over the judged runs when fewer than 23. Bold marks each model's best level. Tasks passed "
        "counts all 23 runs.",
        f"Protocols: Haiku 5.5 is Code quality v3.24 and {NAMES['other']} is {PROTOCOL[OTHER]}. Same rubric, controls, "
        "weights and judges (Muse Spark 1.3 and Grok 4.6, neutral for both labs), judged in separate sessions. "
        + HAIKU_UNJUDGED,
        NOTES[OTHER],
    ]
    yy = y + 0.12
    for n in notes:
        for w in textwrap.wrap(n, 200):
            text(left, yy, w, 10.5, color=MUTED)
            yy += 0.24
        yy += 0.05

    OUTPUT.mkdir(parents=True, exist_ok=True)
    suffix = "" if final else "-preliminary"
    out = OUTPUT / f"haiku55-vs-{OTHER}{suffix}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"haiku55-vs-{OTHER}{suffix}-efforts.csv"
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
                    f"{g['code_quality']['mean']:.4f}",
                    g["passed"],
                    f"{g['usd']['mean']:.4f}",
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
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--vs", choices=sorted(OPPONENTS), required=True)
    select(ap.parse_args().vs)
    main()

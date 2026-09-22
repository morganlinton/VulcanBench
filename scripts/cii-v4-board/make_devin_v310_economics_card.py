"""Companion card to the Devin SWE-2 v3.10 score card: token use and runtime by effort.

SWE-2 has no public API price, so nothing is priced. Reads the frozen population (private manifest: run ids, receipts,
durations) and the population record's per-run API-equivalent costs. Nothing
is re-priced here; every number is a mean over the per-run estimates, and the
pricing caveats are printed on the card.
"""

from __future__ import annotations

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
from harness.retrospective_judging import digest, save  # noqa: E402

LEVELS = ("medium", "high", "max")

RUN = ROOT / "runs-code-quality-maintenance-v3.10"
OUTPUT = ROOT / "docs/results/swe-v4-devin-swe2-2026-09"
LEDGER = OUTPUT / "comparison.json"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {"swe2": "#3B6FE0"}
LEVELS_FOR = {"swe2": tuple(LEVELS)}
NAMES = {"swe2": "Devin SWE-2"}
HARNESS = {"swe2": "Devin CLI"}
# Every finished run of the sweep is counted; the one run that reached the task budget
# (high, cellarcore) is excluded from judging and from these means alike.
EXPECTED = {"swe2/high": 22}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = list(values)
    return {
        "n": len(values),
        "mean": statistics.mean(values),
        "se": statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0,
    }


def load():
    record = json.loads(LEDGER.read_text())
    require(record["models"] == {"swe2": "SWE-2 through the Devin CLI"}, "wrong population")
    require(len(record["rows"]) == 68 and len(record["excluded"]) == 1, "population size")
    rows = []
    for run in record["rows"]:
        receipt = run.get("solver_receipt") or {}
        require(type(receipt.get("raw_tokens")) is int, f"no raw token receipt for {run['run_id']}")
        require(run["api_equivalent_cost_usd"] is None, "SWE-2 is unpriced; a price appeared")
        rows.append(
            {
                "run_id": run["run_id"],
                "model": run["model"],
                "effort": run["effort"],
                "task": run["task"],
                "fallback": bool(run.get("fallback")),
                "tokens": receipt["raw_tokens"],
                "output_tokens": receipt["usage"]["output_tokens"],
                "credits": receipt.get("devin_credit_cost") or 0,
                "acu": receipt.get("devin_acu_cost") or 0.0,
                "minutes": run["duration_s"] / 60,
            }
        )
    ledger = {
        "pricing_verified": None,
        "sources": {
            "devin": "Devin CLI receipts (cli-agent-stream.jsonl); SWE-2 has no public API price"
        },
        "limitations": [
            "SWE-2 has no public API price, so no API-equivalent cost is estimated; the sweep ran on a Devin subscription.",
            "Tokens are the Devin CLI's per-request usage receipts, deduplicated by request id: uncached input, cache reads and output.",
            "Devin's own credit and ACU counters are recorded from the receipts and were zero for every run of this sweep.",
        ],
    }
    return ledger, rows


def aggregate(rows):
    groups = {}
    for model in COLORS:
        for effort in LEVELS_FOR[model]:
            rs = [r for r in rows if r["model"] == model and r["effort"] == effort]
            require(
                len(rs) == EXPECTED.get(f"{model}/{effort}", 23),
                f"{model} {effort} has {len(rs)} rows",
            )
            groups[model, effort] = {
                "model": model,
                "effort": effort,
                "n": len(rs),
                "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
                "output_tokens": mean_se(r["output_tokens"] / 1e3 for r in rs),
                "tokens_total": sum(r["tokens"] for r in rs),
                "minutes": mean_se(r["minutes"] for r in rs),
                "credits": sum(r["credits"] for r in rs),
                "fallbacks": sum(r["fallback"] for r in rs),
            }
    totals = {
        m: {
            "tokens": sum(r["tokens"] for r in rows if r["model"] == m),
            "output_tokens": sum(r["output_tokens"] for r in rows if r["model"] == m),
            "hours": sum(r["minutes"] for r in rows if r["model"] == m) / 60,
            "runs": sum(1 for r in rows if r["model"] == m),
        }
        for m in COLORS
    }
    return groups, totals


def main():  # noqa: PLR0915, one linear figure
    ledger, rows = load()
    groups, totals = aggregate(rows)

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    width_in, height_in = 16, 11.5
    fig = plt.figure(figsize=(width_in, height_in), dpi=150, facecolor=PAPER)

    def yf(inches):
        return 1 - inches / height_in

    def text(
        x, y_in, label, size=16, bold=False, heading=False, numeric=False, ha="left", color=INK
    ):
        family = (
            "IBM Plex Mono"
            if numeric
            else ("Chakra Petch SemiBold" if bold else "Chakra Petch Medium")
            if heading
            else "Geist"
        )
        weight = (
            (500 if bold else 400)
            if numeric
            else ((600 if bold else 500) if heading else (700 if bold else 400))
        )
        return fig.text(
            x,
            yf(y_in),
            label,
            fontsize=size,
            fontfamily=family,
            weight=weight,
            ha=ha,
            va="center",
            color=color,
        )

    def line(x1, x2, y_in, color=RULE, width=0.8):
        fig.add_artist(
            plt.Line2D(
                [x1, x2], [yf(y_in), yf(y_in)], transform=fig.transFigure, color=color, lw=width
            )
        )

    left, right = 0.06, 0.94
    logo_h = 0.42 / height_in
    logo = fig.add_axes([left, yf(0.86), 0.42 / width_in, logo_h])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(left + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(right, 0.65, "September 2026", 14, ha="right", color=MUTED)
    line(left, right, 1.05, INK, 1.2)

    text(
        left,
        1.78,
        "VulcanBench Frontier v4: Devin SWE-2 across effort levels",
        33,
        True,
        heading=True,
    )
    text(
        left,
        2.22,
        "Token use and runtime at every effort level SWE-2 offers, 68 finished runs; "
        "solver inference only, judging excluded. SWE-2 has no public API price.",
        15,
        color=MUTED,
    )

    markers = {"swe2": "o"}
    for model, x in (("swe2", left),):
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.68)],
                transform=fig.transFigure,
                marker=markers[model],
                color=COLORS[model],
                markersize=8,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        text(x + 0.018, 2.68, f"{NAMES[model]}  ·  {HARNESS[model]}", 15, True)
    text(
        right,
        2.68,
        "n=23 at medium and max, n=22 at high",
        13,
        ha="right",
        color=MUTED,
    )

    chart_top, chart_h = 3.75, 2.7
    for panel, (x0, w) in (("output_tokens", (0.085, 0.405)), ("tokens", (0.565, 0.39))):
        tx = left if panel == "output_tokens" else x0 - 0.04
        text(
            tx,
            3.1,
            "Completion tokens per task" if panel == "output_tokens" else "Tokens per task",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "Thousands of output tokens, the model's own writing  ·  lower is better"
            if panel == "output_tokens"
            else "Millions of raw tokens, cache reads included  ·  lower is better",
            12,
            color=MUTED,
        )
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / height_in], facecolor=PAPER)
        ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        peak = max(
            groups[m, e][panel]["mean"] + groups[m, e][panel]["se"]
            for m in COLORS
            for e in LEVELS_FOR[m]
        )
        step = next(s for s in (0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500) if peak / s <= 6)
        top = step * math.ceil(peak / step) + step / 2
        ax.set_ylim(0, top)
        ticks = [i * step for i in range(int(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])
        ax.set_xlim(-0.6, len(LEVELS) - 0.4)
        for model, shift in (("swe2", 0.0),):
            xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
            ys = [groups[model, e][panel]["mean"] for e in LEVELS_FOR[model]]
            es = [groups[model, e][panel]["se"] for e in LEVELS_FOR[model]]
            ax.bar(
                [i + shift for i in xs],
                ys,
                width=0.5,
                color=COLORS[model],
                edgecolor=INK,
                linewidth=0.5,
                yerr=es,
                error_kw={"ecolor": INK, "elinewidth": 0.9, "capsize": 3},
                zorder=2,
            )
            for i, (y, e) in zip(xs, zip(ys, es, strict=True), strict=True):
                label = f"{y:.0f}K" if panel == "output_tokens" else f"{y:.2f}M"
                ax.annotate(
                    label,
                    (i + shift, y + e),
                    xytext=(0, 5),
                    textcoords="offset points",
                    ha="center",
                    va="bottom",
                    fontsize=10,
                    fontfamily="IBM Plex Mono",
                    color=INK,
                )

    # Table: cost, tokens and time at each effort, then the whole sweep
    t0 = 7.15
    text(
        left,
        t0,
        "Table 1  |  Tokens and runtime at each effort level",
        14.5,
        False,
        heading=True,
    )
    cols = {"n": 0.36, "out": 0.52, "total": 0.66, "tok": 0.80, "min": 0.94}
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Effort", 12.5, True)
    for key, label in (
        ("n", "Runs"),
        ("out", "Output/task"),
        ("total", "Level total"),
        ("tok", "Tokens/task"),
        ("min", "Minutes/task"),
    ):
        text(cols[key], t0 + 0.52, label, 12.5, True, ha="right")
    text(cols["out"], t0 + 0.78, "thousands", 10, ha="right", color=MUTED)
    text(cols["total"], t0 + 0.78, "raw tokens, millions", 10, ha="right", color=MUTED)
    text(cols["tok"], t0 + 0.78, "raw, millions", 10, ha="right", color=MUTED)
    text(cols["min"], t0 + 0.78, "solver wall-clock", 10, ha="right", color=MUTED)
    line(left, right, t0 + 0.95, INK, 0.6)
    step, y = 0.35, t0 + 1.25
    for effort in LEVELS:
        g = groups["swe2", effort]
        text(left, y, effort.replace("-", " ").capitalize(), 12.5)
        for key, value in (
            ("n", str(g["n"])),
            ("out", f"{g['output_tokens']['mean']:.0f}K"),
            ("total", f"{g['tokens_total'] / 1e6:,.0f}M"),
            ("tok", f"{g['tokens']['mean']:.2f}M"),
            ("min", f"{g['minutes']['mean']:.1f}"),
        ):
            text(cols[key], y, value, 13.5, numeric=True, ha="right")
        y += step
    line(left, right, y - step / 2, RULE, 0.6)
    tt = totals["swe2"]
    text(left, y, "Full sweep", 13, True)
    for key, value in (
        ("n", str(tt["runs"])),
        ("out", f"{tt['output_tokens'] / tt['runs'] / 1e3:.0f}K"),
        ("total", f"{tt['tokens'] / 1e6:,.0f}M"),
        ("tok", f"{tt['tokens'] / tt['runs'] / 1e6:.2f}M"),
        ("min", f"{tt['hours']:.1f} h"),
    ):
        text(cols[key], y, value, 14.5, True, numeric=True, ha="right")
    y += step
    line(left, right, y - step / 2, INK, 1.2)
    notes = [
        "No cost is shown: Devin publishes no API price for SWE-2 and the sweep ran on a subscription. Devin's credit and ACU "
        "counters in the receipts were zero for every run.",
        "High has 22 runs: cellarcore reached the 3-hour task budget before verification and is excluded from judging and "
        "from these means; the sweep counts it as a fail.",
        "Whiskers are one task standard error. Tokens are raw solver totals including cache reads.",
    ]
    for i, note in enumerate(notes):
        text(left, y + 0.06 + 0.26 * i, note, 11, color=MUTED)

    out = OUTPUT / "devin-swe2-v310-economics.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "devin-swe2-v310-economics-efforts.csv"
    with table.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "n",
                "mean_output_tokens",
                "mean_tokens",
                "se_tokens",
                "total_tokens",
                "mean_minutes",
                "fallbacks",
            ]
        )
        for (model, effort), g in groups.items():
            writer.writerow(
                [
                    model,
                    effort,
                    g["n"],
                    f"{g['output_tokens']['mean'] * 1e3:.0f}",
                    f"{g['tokens']['mean'] * 1e6:.0f}",
                    f"{g['tokens']['se'] * 1e6:.0f}",
                    g["tokens_total"],
                    f"{g['minutes']['mean']:.4f}",
                    g["fallbacks"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "population": f"Devin SWE-2 sweep population record, {len(rows)} finished runs",
            "ledger": LEDGER.name,
            "ledger_sha256": digest(LEDGER.read_bytes()),
            "pricing_verified": ledger["pricing_verified"],
            "sources": ledger["sources"],
            "totals": totals,
            "groups": {f"{m}/{e}": g for (m, e), g in groups.items()},
            "limitations": ledger["limitations"],
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

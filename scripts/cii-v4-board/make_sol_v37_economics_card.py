"""Companion card to the v3.7 Sol score card: API-equivalent cost and token use by effort.

Reads the frozen v3.7 population (private manifest: run ids, receipts,
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
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

RUN = ROOT / "runs-code-quality-maintenance-v3.7"
OUTPUT = ROOT / "docs/results/swe-v4-sol-2026-09"
LEDGER = OUTPUT / "comparison.json"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {"sol": "#10A37F"}
LEVELS_FOR = {"sol": tuple(LEVELS)}
NAMES = {"sol": "GPT-5.6 Sol"}
HARNESS = {"sol": "Codex"}
# Cost is a fact of the sweep, so all 115 runs are priced here, including the
# Max run whose Code quality score the v3.7 summary leaves unpublished.
EXPECTED = {}
PRICES = {"input": 4.00, "cached": 0.40, "output": 20.00}  # Sol list, USD per million


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
    summary = json.loads((RUN / "summary.json").read_text())
    manifest = {r["id"]: r for r in json.loads((RUN / "private-manifest.json").read_text())}
    record = json.loads(LEDGER.read_text())
    priced = {r["run_id"]: r for r in record["rows"]}
    require(summary["protocol"] == "code-quality-maintenance-v3.7", "wrong protocol")
    require(set(summary["passing_panels"]) == {"muse", "grok"}, "panels")
    require(summary["expected_submissions"] == len(summary["rows"]) == 115, "population size")
    rows = []
    for entry in summary["rows"]:
        run = manifest[entry["id"]]
        cost = priced[run["run_id"]]
        require(
            cost["model"] == entry["model"] and cost["effort"] == entry["effort"], "record mismatch"
        )
        receipt = run.get("solver_receipt") or {}
        require(type(receipt.get("raw_tokens")) is int, f"no raw token receipt for {run['run_id']}")
        # The frozen population record carries the cost the sweep stamped at run
        # time; the card recomputes it from the receipt at the published rates.
        usage = receipt["usage"]
        cached = min(usage["cached_input_tokens"], usage["input_tokens"])
        expected = (
            (usage["input_tokens"] - cached) * PRICES["input"]
            + cached * PRICES["cached"]
            + usage["output_tokens"] * PRICES["output"]
        ) / 1e6
        require(
            abs(cost["api_equivalent_cost_usd"] - expected) < 1e-5,
            f"price drift on {run['run_id']}",
        )
        rows.append(
            {
                "run_id": run["run_id"],
                "model": entry["model"],
                "effort": entry["effort"],
                "task": entry["task"],
                "fallback": bool(run.get("fallback")),
                "tokens": receipt["raw_tokens"],
                "usd": cost["api_equivalent_cost_usd"],
                "usd_upper": cost["api_equivalent_cost_usd"],
                "minutes": run["duration_s"] / 60,
            }
        )
    require(len(rows) == 115, f"{len(rows)} priced rows")
    ledger = {
        "pricing_verified": "2026-09-11",
        "sources": {"openai": "https://developers.openai.com/api/docs/pricing"},
        "totals_usd": {m: sum(r["usd"] for r in rows if r["model"] == m) for m in COLORS},
        "limitations": [
            "Codex receipts report cumulative input, cached input and output per run; cached input is priced at the "
            "cached rate ($0.40 per million for Sol against $4.00 uncached, $20.00 output).",
            "Standard tier, short-context rates; no long-context premium is inferred.",
            "Estimates cover exposed receipts, not an independently observed API invoice.",
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
                "usd": mean_se(r["usd"] for r in rs),
                "usd_upper": mean_se(r["usd_upper"] for r in rs),
                "usd_total": sum(r["usd"] for r in rs),
                "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
                "tokens_total": sum(r["tokens"] for r in rs),
                "minutes": mean_se(r["minutes"] for r in rs),
                "fallbacks": sum(r["fallback"] for r in rs),
            }
    totals = {
        m: {
            "usd": sum(r["usd"] for r in rows if r["model"] == m),
            "usd_upper": sum(r["usd_upper"] for r in rows if r["model"] == m),
            "tokens": sum(r["tokens"] for r in rows if r["model"] == m),
            "hours": sum(r["minutes"] for r in rows if r["model"] == m) / 60,
            "runs": sum(1 for r in rows if r["model"] == m),
        }
        for m in COLORS
    }
    return groups, totals


def main():  # noqa: PLR0915, one linear figure
    ledger, rows = load()
    groups, totals = aggregate(rows)
    require(abs(totals["sol"]["usd"] - ledger["totals_usd"]["sol"]) < 1e-6, "totals drift")

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
        "VulcanBench Frontier v4: GPT-5.6 Sol across effort levels",
        33,
        True,
        heading=True,
    )
    text(
        left,
        2.22,
        "API-equivalent cost and token use at every effort level. All 115 runs of the sweep; "
        "solver inference only, judging excluded.",
        15,
        color=MUTED,
    )

    markers = {"sol": "o"}
    for model, x in (("sol", left),):
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
        "n=23 at every effort",
        13,
        ha="right",
        color=MUTED,
    )

    chart_top, chart_h = 3.75, 2.7
    for panel, (x0, w) in (("usd", (0.085, 0.405)), ("tokens", (0.565, 0.39))):
        tx = left if panel == "usd" else x0 - 0.04
        text(
            tx,
            3.1,
            "API-equivalent cost" if panel == "usd" else "Tokens per task",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "USD per task at list prices  ·  lower is better"
            if panel == "usd"
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
        step = next(s for s in (0.5, 1, 2, 5, 10, 20) if peak / s <= 6)
        top = step * math.ceil(peak / step) + step / 2
        ax.set_ylim(0, top)
        ticks = [i * step for i in range(int(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])
        ax.set_xlim(-0.6, len(LEVELS) - 0.4)
        for model, shift in (("sol", 0.0),):
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
                label = f"${y:.2f}" if panel == "usd" else f"{y:.2f}M"
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
        "Table 1  |  Cost, tokens and runtime at each effort level",
        14.5,
        False,
        heading=True,
    )
    cols = {"n": 0.36, "usd": 0.52, "total": 0.66, "tok": 0.80, "min": 0.94}
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Effort", 12.5, True)
    for key, label in (
        ("n", "Runs"),
        ("usd", "$/task"),
        ("total", "Level total"),
        ("tok", "Tokens/task"),
        ("min", "Minutes/task"),
    ):
        text(cols[key], t0 + 0.52, label, 12.5, True, ha="right")
    text(cols["total"], t0 + 0.78, "API-equivalent, list prices", 10, ha="right", color=MUTED)
    text(cols["tok"], t0 + 0.78, "raw, millions", 10, ha="right", color=MUTED)
    text(cols["min"], t0 + 0.78, "solver wall-clock", 10, ha="right", color=MUTED)
    line(left, right, t0 + 0.95, INK, 0.6)
    step, y = 0.35, t0 + 1.25
    for effort in LEVELS:
        g = groups["sol", effort]
        text(left, y, effort.replace("-", " ").capitalize(), 12.5)
        for key, value in (
            ("n", str(g["n"])),
            ("usd", f"${g['usd']['mean']:.2f}"),
            ("total", f"${g['usd_total']:.2f}"),
            ("tok", f"{g['tokens']['mean']:.2f}M"),
            ("min", f"{g['minutes']['mean']:.1f}"),
        ):
            text(cols[key], y, value, 13.5, numeric=True, ha="right")
        y += step
    line(left, right, y - step / 2, RULE, 0.6)
    tt = totals["sol"]
    text(left, y, "Full sweep", 13, True)
    for key, value in (
        ("n", str(tt["runs"])),
        ("usd", f"${tt['usd'] / tt['runs']:.2f}"),
        ("total", f"${tt['usd']:,.2f}"),
        ("tok", f"{tt['tokens'] / 1e6:,.0f}M"),
        ("min", f"{tt['hours']:.1f} h"),
    ):
        text(cols[key], y, value, 14.5, True, numeric=True, ha="right")
    y += step
    line(left, right, y - step / 2, INK, 1.2)
    notes = [
        f"USD at list prices checked {ledger['pricing_verified']}, cache-aware (Sol \\$4.00 input, \\$0.40 cached input, \\$20.00 output per million), "
        "solver inference only; subscription bills differ.",
        "All 115 runs are priced. The score card publishes 114: one Max run (codeccore) carries no Code quality score "
        "because a judge probe quoted an excerpt absent from the code on both attempts.",
        "Whiskers are one task standard error. Tokens are raw solver totals including cache reads.",
    ]
    for i, note in enumerate(notes):
        text(left, y + 0.06 + 0.26 * i, note, 11, color=MUTED)

    out = OUTPUT / "sol-v37-economics.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "sol-v37-economics-efforts.csv"
    with table.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "n",
                "mean_usd",
                "se_usd",
                "total_usd",
                "long_context_upper_mean_usd",
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
                    f"{g['usd']['mean']:.6f}",
                    f"{g['usd']['se']:.6f}",
                    f"{g['usd_total']:.6f}",
                    f"{g['usd_upper']['mean']:.6f}",
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
            "population": f"runs-code-quality-maintenance-v3.7 population, {len(rows)} runs",
            "ledger": LEDGER.name,
            "ledger_sha256": digest(LEDGER.read_bytes()),
            "pricing_verified": ledger["pricing_verified"],
            "sources": ledger["sources"],
            "totals": totals,
            "groups": {f"{m}/{e}": g for (m, e), g in groups.items()},
            "limitations": ledger["limitations"],
            "summary_sha256": digest((RUN / "summary.json").read_bytes()),
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

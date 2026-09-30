"""GPT-6 against GPT-5.6 on VulcanBench Frontier v4: three comparison cards.

- gpt6-vs-gpt56-luna: GPT-6 Luna beside GPT-5.6 Luna
- gpt6-vs-gpt56-sol: GPT-6 Sol beside GPT-5.6 Sol
- gpt6-vs-gpt56-all: all four

Reads the frozen Code quality summaries that already published each model
(v3.5 for GPT-5.6 Luna, v3.7 for GPT-5.6 Sol, v3.16 for GPT-6 Luna, v3.17 for
GPT-6 Sol; every pass judged by Muse Spark 1.3 and Grok 4.6 under one rubric,
controls, gates and calibration exam), their private manifests and the
population records those reports used as cost ledgers. Nothing is re-judged.

Combined scores are means over judged runs, as on the per-model cards. Tasks
passed, minutes and cost cover every run in the cell: a run whose Code quality
is unpublished (GPT-5.6 Sol max codeccore, an invalid judge probe) still counts
its pass, time and cost, and a run stopped at the 3-hour bound (GPT-6 Luna,
extra-high 2, max 4) counts as a failed task at its recorded time and is
unpriced. For GPT-6 Luna a dashed line also shows the combined score with each
timeout counted as 0 over all 23 runs (owner decision, DECISIONS.md 2026-09-28).
Every cost is recomputed from the run's receipt at the model's list price and
must match the ledger's stamp.

    python scripts/cii-v4-board/make_gpt6_vs_gpt56_cards.py [--only luna|sol|all]
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
from harness.evaluator.reviewed_score import WEIGHTS_V3  # noqa: E402
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

OUTPUT = ROOT / "docs/results/swe-v4-gpt6-vs-gpt56-2026-09"
RESULTS = ROOT / "docs/results"
# key: (judging run, protocol id, key in that run, population record, list rates in/cached/out per million)
SOURCES = {
    "gpt6sol": (
        "runs-code-quality-maintenance-v3.17",
        "code-quality-maintenance-v3.17",
        "gpt6sol",
        "swe-v4-gpt6-sol-2026-09/comparison.json",
        (2.00, 0.20, 10.00),
    ),
    "sol56": (
        "runs-code-quality-maintenance-v3.7",
        "code-quality-maintenance-v3.7",
        "sol",
        "swe-v4-sol-2026-09/comparison.json",
        (4.00, 0.40, 20.00),
    ),
    "gpt6luna": (
        "runs-code-quality-maintenance-v3.16",
        "code-quality-maintenance-v3.16",
        "gpt6luna",
        "swe-v4-gpt6-luna-2026-09/comparison.json",
        (0.10, 0.01, 0.50),
    ),
    "luna56": (
        "runs-code-quality-maintenance-v3.5",
        "code-quality-maintenance-v3.5",
        "luna",
        "swe-v4-gpt55-luna-2026-09/comparison.json",
        (0.20, 0.02, 1.20),
    ),
}
NAMES = {
    "gpt6sol": "GPT-6 Sol",
    "sol56": "GPT-5.6 Sol",
    "gpt6luna": "GPT-6 Luna",
    "luna56": "GPT-5.6 Luna",
}
HARNESS = {
    "gpt6sol": "Codex 0.155.0",
    "sol56": "Codex",
    "gpt6luna": "Codex 0.155.0",
    "luna56": "Codex",
}
COLORS = {"gpt6sol": "#10A37F", "sol56": "#7A9A2E", "gpt6luna": "#0F5E4F", "luna56": "#5EC59B"}
MARKERS = {"gpt6sol": "D", "sol56": "D", "gpt6luna": "v", "luna56": "v"}
HOLLOW = {"sol56", "luna56"}  # previous generation: hollow markers
# Judged rows per cell where fewer than 23; every other cell is 23.
JUDGED = {
    ("sol56", "max"): 22,
    ("gpt6sol", "medium"): 22,
    ("gpt6luna", "extra-high"): 21,
    ("gpt6luna", "max"): 19,
}
# Cells whose unjudged runs are 3-hour timeouts (they get the timeouts-as-0 figure).
TIMEOUTS = {("gpt6luna", "extra-high"): 2, ("gpt6luna", "max"): 4}
CARDS = {
    "luna": ("gpt6-vs-gpt56-luna", ("gpt6luna", "luna56"), "GPT-6 Luna vs. GPT-5.6 Luna"),
    "sol": ("gpt6-vs-gpt56-sol", ("gpt6sol", "sol56"), "GPT-6 Sol vs. GPT-5.6 Sol"),
    "all": (
        "gpt6-vs-gpt56-all",
        ("gpt6sol", "sol56", "gpt6luna", "luna56"),
        "GPT-6 Sol and Luna vs. GPT-5.6",
    ),
}
PANELS = ("muse", "grok")
SPLIT_WITHOUT_L3 = {"l1": 0.24, "l2": 0.09}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
PRICING_VERIFIED = {
    "gpt6sol": "2026-09-25",
    "gpt6luna": "2026-09-25",
    "sol56": "2026-09-11",
    "luna56": "2026-09-11",
}


def require(condition, message):
    if not condition:
        raise SystemExit(f"Card refused: {message}")


def mean_se(values):
    values = list(values)
    if not values:
        return {"n": 0, "mean": None, "se": None}
    return {
        "n": len(values),
        "mean": statistics.mean(values),
        "se": statistics.stdev(values) / math.sqrt(len(values)) if len(values) > 1 else 0.0,
    }


def code_quality(row):
    panels = [row["panels"][p] for p in PANELS]
    require(all(p["l1"] is not None for p in panels), f"{row['id']}: reviews incomplete")
    l1 = statistics.mean(p["l1"]["score"] for p in panels)
    l2_values = [p["l2"] for p in panels if p["l2"] is not None]
    require(
        all(p["l2"] is not None or p["l2_denominator"] == 0 for p in panels),
        f"{row['id']}: probes incomplete",
    )
    if l2_values:
        return (
            SPLIT_WITHOUT_L3["l1"] * l1 + SPLIT_WITHOUT_L3["l2"] * statistics.mean(l2_values)
        ) / WEIGHTS_V3["human_like"]
    return l1


def composite(run, quality):
    other = (0.5 - WEIGHTS_V3["human_like"]) / 2
    return 100 * (
        0.5 * run["functional"]
        + other * run["quality"]
        + other * run["security"]
        + WEIGHTS_V3["human_like"] * quality / 100
    )


def receipt_cost(rates, receipt):
    usage = receipt["usage"]
    cached = min(usage["cached_input_tokens"], usage["input_tokens"])
    inp, cch, out = rates
    return (
        (usage["input_tokens"] - cached) * inp + cached * cch + usage["output_tokens"] * out
    ) / 1e6


def load(model):
    run_name, protocol_id, key, ledger_name, rates = SOURCES[model]
    run_dir = ROOT / run_name
    summary = json.loads((run_dir / "summary.json").read_text())
    protocol = json.loads((run_dir / "protocol.json").read_text())
    ledger_path = RESULTS / ledger_name
    ledger = json.loads(ledger_path.read_text())
    require(summary["protocol"] == protocol_id, f"{run_name}: wrong protocol")
    require(set(summary["passing_panels"]) == set(PANELS), f"{run_name}: panels")
    require(
        protocol["weights"]["functional"] == WEIGHTS_V3["functional"]
        and protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"],
        f"{run_name}: weights drift",
    )
    manifest = {r["id"]: r for r in json.loads((run_dir / "private-manifest.json").read_text())}
    judged = {}
    for entry in summary["rows"]:
        if entry["model"] != key or not entry.get("published"):
            continue
        run = manifest[entry["id"]]
        judged[run["run_id"]] = composite(run, code_quality(entry))
    runs = []
    for r in ledger["rows"]:
        if r["model"] != key:
            continue
        receipt = r.get("solver_receipt") or {}
        require(type(receipt.get("raw_tokens")) is int, f"no receipt for {r['run_id']}")
        usd = receipt_cost(rates, receipt)
        require(abs(r["api_equivalent_cost_usd"] - usd) < 1e-5, f"price drift on {r['run_id']}")
        runs.append(
            {
                "effort": r["effort"],
                "task": r["task"],
                "run_id": r["run_id"],
                "passed": r["functional"] == 1,
                "minutes": r["duration_s"] / 60,
                "usd": usd,
                "combined": judged.get(r["run_id"]),
                "timeout": False,
            }
        )
    for e in ledger.get("excluded", []):
        if e["model"] != key:
            continue
        require(
            e["finished"] is False and e["duration_s"] >= 10700,
            f"{model}: exclusion {e['task']} is not a 3-hour timeout",
        )
        runs.append(
            {
                "effort": e["effort"],
                "task": e["task"],
                "run_id": e["run_id"],
                "passed": False,
                "minutes": e["duration_s"] / 60,
                "usd": None,
                "combined": None,
                "timeout": True,
            }
        )
    require(
        not [m for m in ledger.get("missing", []) if m["model"] == key], f"{model}: missing runs"
    )
    hashes = {
        "summary": digest((run_dir / "summary.json").read_bytes()),
        "protocol": digest((run_dir / "protocol.json").read_bytes()),
        "ledger": digest(ledger_path.read_bytes()),
    }
    return runs, hashes


def aggregate(model, runs):
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        require(len(rs) == 23, f"{model} {effort}: {len(rs)} runs")
        scored = [r["combined"] for r in rs if r["combined"] is not None]
        require(
            len(scored) == JUDGED.get((model, effort), 23),
            f"{model} {effort}: {len(scored)} judged rows",
        )
        timeouts = sum(r["timeout"] for r in rs)
        require(timeouts == TIMEOUTS.get((model, effort), 0), f"{model} {effort}: timeouts")
        priced = [r["usd"] for r in rs if r["usd"] is not None]
        groups[effort] = {
            "n_runs": len(rs),
            "n_judged": len(scored),
            "timeouts": timeouts,
            "combined": mean_se(scored),
            "combined_timeouts_zero": mean_se(scored + [0.0] * timeouts) if timeouts else None,
            "passed": sum(r["passed"] for r in rs),
            "minutes": mean_se(r["minutes"] for r in rs),
            "usd": mean_se(priced),
            "n_priced": len(priced),
        }
    return groups


WIDTH_IN = 16


def draw(card, models, title, groups, hashes):  # noqa: PLR0912, PLR0915, one linear figure
    rows_per_model = 4
    height_in = 9.35 + 0.33 * rows_per_model * len(models) + 0.26 * (5 if len(models) > 2 else 4)
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    fig = plt.figure(figsize=(WIDTH_IN, height_in), dpi=150, facecolor=PAPER)

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

    def marker_style(model):
        face = PAPER if model in HOLLOW else COLORS[model]
        return {
            "marker": MARKERS[model],
            "markersize": 8,
            "markerfacecolor": face,
            "markeredgecolor": COLORS[model] if model in HOLLOW else INK,
            "markeredgewidth": 1.4 if model in HOLLOW else 0.6,
        }

    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / WIDTH_IN, 0.42 / height_in])
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
    text(left, 1.78, f"VulcanBench Frontier v4: {title}", 33, True, heading=True)
    text(
        left,
        2.22,
        "Combined score and cost per task at every effort level, 23 tasks per effort. "
        "Code quality judged by Muse Spark 1.3 and Grok 4.6.",
        15,
        color=MUTED,
    )

    # Legend
    step_x = (right - left) / len(models)
    for i, model in enumerate(models):
        x = left + i * step_x
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.68)],
                transform=fig.transFigure,
                color=COLORS[model],
                linestyle="none",
                **marker_style(model),
            )
        )
        text(x + 0.018, 2.68, f"{NAMES[model]}  ·  {HARNESS[model]}", 13.5, True)

    # Charts
    chart_top, chart_h = 3.75, 3.0
    xs = list(range(len(LEVELS)))
    offsets = {m: (i - (len(models) - 1) / 2) * 0.07 for i, m in enumerate(models)}
    for panel, (x0, w) in (("combined", (0.085, 0.405)), ("usd", (0.585, 0.37))):
        tx = left if panel == "combined" else x0 - 0.06
        text(
            tx,
            3.1,
            "Combined score" if panel == "combined" else "API-equivalent cost",
            21,
            True,
            heading=True,
        )
        sub = (
            "/100  ·  higher is better  ·  dashed: GPT-6 Luna with timeouts as 0"
            if panel == "combined" and "gpt6luna" in models
            else "/100  ·  higher is better"
            if panel == "combined"
            else "USD per priced task at list prices  ·  log scale  ·  lower is better"
        )
        text(tx, 3.42, sub, 12, color=MUTED)
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / height_in], facecolor=PAPER)
        ax.set_xticks(xs, [e.replace("-", " ").capitalize() for e in LEVELS])
        ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
        ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
        ax.spines[["top", "right"]].set_visible(False)
        ax.spines[["left", "bottom"]].set_color(RULE)
        ax.grid(axis="y", color=RULE, linewidth=0.5)
        ax.set_axisbelow(True)
        ax.set_xlim(-0.45, len(LEVELS) - 0.55)
        for model in models:
            g = groups[model]
            px = [x + offsets[model] for x in xs]
            ys = [g[e][panel]["mean"] for e in LEVELS]
            es = [g[e][panel]["se"] or 0 for e in LEVELS]
            ax.plot(px, ys, color=COLORS[model], linewidth=2, zorder=2)
            ax.errorbar(
                px, ys, yerr=es, fmt="none", ecolor=INK, elinewidth=0.8, capsize=2.5, zorder=3
            )
            ax.plot(px, ys, linestyle="none", zorder=4, color=COLORS[model], **marker_style(model))
            if panel == "combined" and any(g[e]["combined_timeouts_zero"] for e in LEVELS):
                zs = [(g[e]["combined_timeouts_zero"] or g[e]["combined"])["mean"] for e in LEVELS]
                ax.plot(px, zs, color=COLORS[model], linewidth=1.5, linestyle=(0, (4, 3)), zorder=1)
        if panel == "usd":
            ax.set_yscale("log")
            lo = min(g[e]["usd"]["mean"] for m in models for g in [groups[m]] for e in LEVELS)
            hi = max(g[e]["usd"]["mean"] for m in models for g in [groups[m]] for e in LEVELS)
            ticks = [t for t in (0.003, 0.01, 0.03, 0.1, 0.3, 1, 3, 10) if lo / 3 <= t <= hi * 3]
            ax.yaxis.set_major_locator(matplotlib.ticker.FixedLocator(ticks))
            ax.yaxis.set_minor_locator(matplotlib.ticker.NullLocator())
            ax.yaxis.set_major_formatter(matplotlib.ticker.FuncFormatter(lambda v, _: f"\\${v:g}"))

    # Table
    t0 = 7.45
    text(left, t0, "Table 1  |  Each model at each effort level", 14.5, False, heading=True)
    cols = {e: x for e, x in zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True)}
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Model and measure", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], t0 + 0.52, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, t0 + 0.7, INK, 0.6)
    y, step = t0 + 0.98, 0.33
    for model in models:
        g = groups[model]
        text(left, y, NAMES[model], 13, True, color=INK)
        fig.add_artist(
            plt.Line2D(
                [left - 0.012],
                [yf(y)],
                transform=fig.transFigure,
                color=COLORS[model],
                linestyle="none",
                **{**marker_style(model), "markersize": 7},
            )
        )
        measures = [
            ("Combined score", lambda c: f"{c['combined']['mean']:.2f}"),
            ("Tasks passed, of 23", lambda c: f"{c['passed']}"),
            (
                "$ per priced task",
                lambda c: (
                    f"${c['usd']['mean']:.3f}"
                    if c["usd"]["mean"] < 1
                    else f"${c['usd']['mean']:.2f}"
                ),
            ),
            ("Minutes per task", lambda c: f"{c['minutes']['mean']:.1f}"),
        ]
        for label, fmt in measures:
            if label != "Combined score":
                text(left + 0.02, y, label, 12, color=INK)
            else:
                text(left + 0.16, y, label, 12, color=MUTED)
            for effort in LEVELS:
                cell = g[effort]
                value = fmt(cell)
                if label == "Combined score" and cell["combined_timeouts_zero"]:
                    value = f"{value} / {cell['combined_timeouts_zero']['mean']:.2f}"
                text(cols[effort], y, value, 13, numeric=True, ha="right")
            y += step
        line(left, right, y - step / 2, RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)

    notes = [
        "Combined score averages judged runs, as on each model's own card; GPT-6 Luna shows a second figure after the slash with its "
        "3-hour timeouts counted as 0 over 23 runs"
        if "gpt6luna" in models
        else "Combined score averages judged runs, as on each model's own card. Tasks passed, cost and minutes cover all 23 runs per cell.",
    ]
    if "gpt6luna" in models:
        notes.append(
            "(judged n=21 at extra high, 19 at max). Tasks passed and minutes cover all 23 runs; timeouts count as failures at 180 minutes "
            "and are unpriced."
        )
    if "sol56" in models:
        notes.append(
            "GPT-5.6 Sol max and GPT-6 Sol medium are each judged on 22 of 23 runs: one invalid judge probe on codeccore in each; "
            "those runs are counted and priced."
        )
    if "luna56" in models:
        notes.append(
            "GPT-5.6 Luna ran before the 3-hour bound (September 13); one of its max passes took 196 minutes."
        )
    notes.append(
        "Each model was judged in its own protocol run ("
        + ", ".join(f"{NAMES[m]} {SOURCES[m][1].rsplit('-', 1)[1]}" for m in models)
        + ") with the same rubric and judges. GPT-6 ran on Codex CLI 0.155.0, GPT-5.6 on 0.153.4 or older."
    )
    prices = [f"{NAMES[m]} \\${SOURCES[m][4][0]:.2f} and \\${SOURCES[m][4][2]:.2f}" for m in models]
    half = (len(prices) + 1) // 2 if len(prices) > 2 else len(prices)
    notes.append(
        "List prices per million tokens, input and output: "
        + "; ".join(prices[:half])
        + (";" if prices[half:] else ".")
    )
    if prices[half:]:
        notes.append(
            "; ".join(prices[half:]) + ". Solver inference only; subscription bills differ."
        )
    else:
        notes[-1] += " Solver inference only; subscription bills differ."
    for i, note in enumerate(notes):
        text(left, y - step / 2 + 0.2 + 0.26 * i, note, 10.5, color=MUTED)

    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / f"{card}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / f"{card}-efforts.csv"
    with table.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "runs",
                "judged",
                "timeouts",
                "combined",
                "combined_se",
                "combined_timeouts_zero",
                "passed",
                "priced_runs",
                "mean_usd",
                "mean_minutes",
            ]
        )
        for model in models:
            for effort in LEVELS:
                c = groups[model][effort]
                writer.writerow(
                    [
                        model,
                        effort,
                        c["n_runs"],
                        c["n_judged"],
                        c["timeouts"],
                        f"{c['combined']['mean']:.4f}",
                        f"{c['combined']['se']:.4f}",
                        f"{c['combined_timeouts_zero']['mean']:.4f}"
                        if c["combined_timeouts_zero"]
                        else "",
                        c["passed"],
                        c["n_priced"],
                        f"{c['usd']['mean']:.6f}",
                        f"{c['minutes']['mean']:.4f}",
                    ]
                )
    save(
        out.with_suffix(".json"),
        {
            "card": card,
            "models": {m: NAMES[m] for m in models},
            "sources": {
                m: {
                    "judging_run": SOURCES[m][0],
                    "protocol": SOURCES[m][1],
                    "ledger": SOURCES[m][3],
                    "list_rates_per_million": SOURCES[m][4],
                    "pricing_verified": PRICING_VERIFIED[m],
                    **hashes[m],
                }
                for m in models
            },
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
            "visual_qa": "Requires visual inspection after rendering",
        },
    )
    print(out)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--only", choices=sorted(CARDS))
    args = parser.parse_args()
    wanted = [args.only] if args.only else list(CARDS)
    needed = {m for c in wanted for m in CARDS[c][1]}
    groups, hashes = {}, {}
    for model in needed:
        runs, hashes[model] = load(model)
        groups[model] = aggregate(model, runs)
    for c in wanted:
        card, models, title = CARDS[c]
        draw(card, models, title, groups, hashes)


if __name__ == "__main__":
    main()

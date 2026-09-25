"""OpenAI's five models on VulcanBench Frontier v4: score card and economics card.

Reads the frozen Code quality summaries that already published each model
(v3.4 for GPT-6 Astra, v3.5 for GPT-5.5 and GPT-5.6 Luna, v3.6 plus the
v3.6.1 top-up for GPT-5.6 Terra, v3.7 for GPT-5.6 Sol; every pass judged by
Muse Spark 1.3 and Grok 4.6 under one rubric, controls, gates and calibration
exam) together with the private manifests and the cost ledgers those reports
used. Nothing is re-judged or re-priced beyond what the per-model cards
already do; the coverage rules of those cards apply row by row.

    python scripts/cii-v4-board/make_openai_family_cards.py

Writes docs/results/swe-v4-openai-family-2026-09/openai-family.png (score)
and openai-family-economics.png (cost and tokens), each with an SVG, a CSV
of the plotted aggregates and a JSON record of sources and hashes.
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
from harness.evaluator.reviewed_score import WEIGHTS_V3  # noqa: E402
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

OUTPUT = ROOT / "docs/results/swe-v4-openai-family-2026-09"
RESULTS = ROOT / "docs/results"
MODELS = ("astra", "sol", "terra", "gpt55", "luna")  # legend order: newest generation first
RUNS = {
    "astra": [ROOT / "runs-code-quality-maintenance-v3.4"],
    "gpt55": [ROOT / "runs-code-quality-maintenance-v3.5"],
    "luna": [ROOT / "runs-code-quality-maintenance-v3.5"],
    "terra": [
        ROOT / "runs-code-quality-maintenance-v3.6",
        ROOT / "runs-code-quality-maintenance-v3.6.1",
    ],
    "sol": [ROOT / "runs-code-quality-maintenance-v3.7"],
}
PROTOCOLS = {
    "astra": {"code-quality-maintenance-v3.4"},
    "gpt55": {"code-quality-maintenance-v3.5"},
    "luna": {"code-quality-maintenance-v3.5"},
    "terra": {"code-quality-maintenance-v3.6", "code-quality-maintenance-v3.6.1"},
    "sol": {"code-quality-maintenance-v3.7"},
}
# The v3.7 summary is not flagged final only because one Max probe was invalid
# and the row is left unpublished; the ops log records the decision.
NOT_FINAL_ALLOWED = {"runs-code-quality-maintenance-v3.7": {"submission-023"}}
LEDGERS = {
    "astra": RESULTS / "swe-v4-astra-fable51-2026-09/api-equivalent-costs.json",
    "gpt55": RESULTS / "swe-v4-gpt55-luna-2026-09/comparison.json",
    "luna": RESULTS / "swe-v4-gpt55-luna-2026-09/comparison.json",
    "terra": RESULTS / "swe-v4-terra-2026-09/comparison.json",
    "sol": RESULTS / "swe-v4-sol-2026-09/comparison.json",
}
TERRA_TOPUP_LEDGER = RESULTS / "swe-v4-terra-2026-09/comparison-topup.json"
# List rates (USD per million; input, cached input, output) the per-model economics
# cards verified on 2026-09-11 against developers.openai.com/api/docs/pricing.
RATES = {
    "gpt55": (5.00, 0.50, 30.00),
    "luna": (0.20, 0.02, 1.20),
    "terra": (2.00, 0.20, 12.00),
    "sol": (4.00, 0.40, 20.00),
}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
COLORS = {
    "astra": "#10A37F",
    "sol": "#7A9A2E",
    "terra": "#0F5E4F",
    "gpt55": "#6B6B66",
    "luna": "#5EC59B",
}
MARKERS = {"astra": "o", "sol": "D", "terra": "s", "gpt55": "^", "luna": "v"}
LEVELS_FOR = {m: tuple(LEVELS) for m in MODELS}
LEVELS_FOR["gpt55"] = ("low", "medium", "high", "extra-high")  # GPT-5.5 has no max level
NAMES = {
    "astra": "GPT-6 Astra",
    "sol": "GPT-5.6 Sol",
    "terra": "GPT-5.6 Terra",
    "gpt55": "GPT-5.5",
    "luna": "GPT-5.6 Luna",
}
SHORT = {"astra": "Astra", "sol": "Sol", "terra": "Terra", "gpt55": "GPT-5.5", "luna": "Luna"}
PANELS = ("muse", "grok")
SPLIT_WITHOUT_L3 = {"l1": 0.24, "l2": 0.09}
EXPECTED = {("sol", "max"): 22}  # judged rows; every other cell is 23


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
    if any(p["l1"] is None for p in panels):
        return None
    l1 = statistics.mean(p["l1"]["score"] for p in panels)
    l2_values = [p["l2"] for p in panels if p["l2"] is not None]
    probes_complete = all(p["l2"] is not None or p["l2_denominator"] == 0 for p in panels)
    require(probes_complete, f"{row['id']}: probes incomplete")
    if l2_values:
        cq = (
            SPLIT_WITHOUT_L3["l1"] * l1 + SPLIT_WITHOUT_L3["l2"] * statistics.mean(l2_values)
        ) / WEIGHTS_V3["human_like"]
        return cq, l1, statistics.mean(l2_values)
    return l1, l1, None


def composite(run, quality, weight):
    other = (0.5 - weight) / 2
    return 100 * (
        0.5 * run["functional"]
        + other * run["quality"]
        + other * run["security"]
        + weight * quality / 100
    )


def receipt_cost(model, receipt):
    usage = receipt["usage"]
    cached = min(usage["cached_input_tokens"], usage["input_tokens"])
    inp, cch, out = RATES[model]
    return (
        (usage["input_tokens"] - cached) * inp + cached * cch + usage["output_tokens"] * out
    ) / 1e6


def load_costs():
    """Per run id: central USD estimate and, for Astra, the long-context upper bound."""
    costs, hashes = {}, {}
    astra = json.loads(LEDGERS["astra"].read_text())
    hashes["astra"] = digest(LEDGERS["astra"].read_bytes())
    for r in astra["rows"]:
        if r["model"] == "astra":
            costs["astra", r["run_id"]] = (r["estimated_usd"], r["long_context_upper_usd"])
    for model in ("gpt55", "luna", "terra", "sol"):
        record = json.loads(LEDGERS[model].read_text())
        hashes[model] = digest(LEDGERS[model].read_bytes())
        rows = [r for r in record["rows"] if r["model"] == model]
        if model == "terra":
            rows += json.loads(TERRA_TOPUP_LEDGER.read_text())["rows"]
            hashes["terra_topup"] = digest(TERRA_TOPUP_LEDGER.read_bytes())
        for r in rows:
            costs[model, r["run_id"]] = (r["api_equivalent_cost_usd"], None)
    return costs, hashes, astra


def load():
    costs, ledger_hashes, astra_ledger = load_costs()
    rows, hashes, notes = [], {}, []
    for model in MODELS:
        for run_dir in RUNS[model]:
            summary = json.loads((run_dir / "summary.json").read_text())
            protocol = json.loads((run_dir / "protocol.json").read_text())
            require(summary["protocol"] in PROTOCOLS[model], f"{run_dir.name}: wrong protocol")
            unpublished = {r["id"] for r in summary["rows"] if not r.get("published")}
            require(
                summary["ready_for_publication"]
                or unpublished == NOT_FINAL_ALLOWED.get(run_dir.name, set()),
                f"{run_dir.name}: not final ({sorted(unpublished)})",
            )
            require(set(summary["passing_panels"]) == set(PANELS), f"{run_dir.name}: panels")
            require(
                protocol["weights"]["functional"] == WEIGHTS_V3["functional"]
                and protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"],
                "weights drift",
            )
            manifest = {
                r["id"]: r for r in json.loads((run_dir / "private-manifest.json").read_text())
            }
            hashes[run_dir.name] = {
                "summary": digest((run_dir / "summary.json").read_bytes()),
                "protocol": digest((run_dir / "protocol.json").read_bytes()),
            }
            for entry in summary["rows"]:
                if entry["model"] != model or not entry.get("published"):
                    continue
                run = manifest[entry["id"]]
                cq, l1, l2 = code_quality(entry)
                receipt = run.get("solver_receipt") or {}
                require(type(receipt.get("raw_tokens")) is int, f"no receipt for {run['run_id']}")
                usd, upper = costs[model, run["run_id"]]
                if model in RATES:
                    recomputed = receipt_cost(model, receipt)
                    if model == "terra":
                        # The Terra population record carries a stale run-time stamp; the
                        # Terra economics card uses the re-priced run summaries. Same here.
                        usd = recomputed
                    else:
                        require(abs(usd - recomputed) < 1e-5, f"price drift on {run['run_id']}")
                rows.append(
                    {
                        "model": model,
                        "effort": entry["effort"],
                        "task": entry["task"],
                        "run_id": run["run_id"],
                        "functional": run["functional"],
                        "minutes": run["duration_s"] / 60,
                        "code_quality": cq,
                        "l1": l1,
                        "l2": l2,
                        "readability": statistics.mean(
                            entry["panels"][p]["l1"]["readability"] for p in PANELS
                        ),
                        "maintainability": statistics.mean(
                            entry["panels"][p]["l1"]["maintainability"] for p in PANELS
                        ),
                        "combined": composite(run, cq, WEIGHTS_V3["human_like"]),
                        "usd": usd,
                        "usd_upper": upper if upper is not None else usd,
                        "tokens": receipt["raw_tokens"],
                        "fallback": bool(run.get("fallback")),
                    }
                )
    notes.append(astra_ledger["pricing_verified"])
    return rows, {"summaries": hashes, "ledgers": ledger_hashes}, astra_ledger


def aggregate(rows):
    groups = {}
    for model in MODELS:
        for effort in LEVELS_FOR[model]:
            rs = [r for r in rows if r["model"] == model and r["effort"] == effort]
            require(
                len(rs) == EXPECTED.get((model, effort), 23), f"{model} {effort}: {len(rs)} rows"
            )
            groups[model, effort] = {
                "model": model,
                "effort": effort,
                "n": len(rs),
                "combined": mean_se(r["combined"] for r in rs),
                "code_quality": mean_se(r["code_quality"] for r in rs),
                "readability": mean_se(r["readability"] for r in rs),
                "maintainability": mean_se(r["maintainability"] for r in rs),
                "l2": mean_se(r["l2"] for r in rs if r["l2"] is not None),
                "minutes": mean_se(r["minutes"] for r in rs),
                "usd": mean_se(r["usd"] for r in rs),
                "usd_upper": mean_se(r["usd_upper"] for r in rs),
                "usd_total": sum(r["usd"] for r in rs),
                "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
                "passed": sum(r["functional"] == 1 for r in rs),
                "fallbacks": sum(r["fallback"] for r in rs),
            }
    return groups


# --- drawing helpers ------------------------------------------------------------

WIDTH_IN, HEIGHT_IN = 16, 11.5


def yf(inches):
    return 1 - inches / HEIGHT_IN


def new_figure():
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    return plt.figure(figsize=(WIDTH_IN, HEIGHT_IN), dpi=150, facecolor=PAPER)


def make_text(fig):
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

    return text, line


LEFT, RIGHT = 0.06, 0.94


def masthead(fig, text, line, title, subtitle):
    logo = fig.add_axes([LEFT, yf(0.86), 0.42 / WIDTH_IN, 0.42 / HEIGHT_IN])
    mark = logo.imshow(plt.imread(ROOT / "docs/assets/vulcanbench-logo.png"))
    mark.set_clip_path(
        FancyBboxPatch(
            (0, 0), 1, 1, boxstyle="round,pad=0,rounding_size=.22", transform=logo.transAxes
        )
    )
    logo.axis("off")
    text(LEFT + 0.038, 0.65, "VulcanBench", 20, True, heading=True)
    text(RIGHT, 0.65, "September 2026", 14, ha="right", color=MUTED)
    line(LEFT, RIGHT, 1.05, INK, 1.2)
    text(LEFT, 1.78, title, 33, True, heading=True)
    text(LEFT, 2.22, subtitle, 15, color=MUTED)
    # Legend: five models across one row
    xs = (LEFT, 0.245, 0.43, 0.615, 0.79)
    for model, x in zip(MODELS, xs, strict=True):
        fig.add_artist(
            plt.Line2D(
                [x + 0.004],
                [yf(2.68)],
                transform=fig.transFigure,
                marker=MARKERS[model],
                color=COLORS[model],
                markersize=8,
                markeredgecolor=INK,
                markeredgewidth=0.6,
                linestyle="none",
            )
        )
        text(x + 0.018, 2.68, NAMES[model], 14.5, True)


def style_axis(ax):
    ax.set_xticks(range(len(LEVELS)), [e.replace("-", " ").capitalize() for e in LEVELS])
    ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
    ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
    ax.spines[["top", "right"]].set_visible(False)
    ax.spines[["left", "bottom"]].set_color(RULE)
    ax.grid(axis="y", color=RULE, linewidth=0.5)
    ax.set_axisbelow(True)


BAR_SHIFTS = {m: s for m, s in zip(MODELS, (-0.32, -0.16, 0.0, 0.16, 0.32), strict=True)}


def grouped_bars(ax, groups, key, label_fmt, upper_key=None):
    peak = 0.0
    for model in MODELS:
        xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
        ys = [groups[model, e][key]["mean"] for e in LEVELS_FOR[model]]
        es = [groups[model, e][key]["se"] or 0 for e in LEVELS_FOR[model]]
        peak = max(peak, *(y + e for y, e in zip(ys, es, strict=True)))
        ax.bar(
            [i + BAR_SHIFTS[model] for i in xs],
            ys,
            width=0.15,
            color=COLORS[model],
            edgecolor=INK,
            linewidth=0.4,
            yerr=es,
            error_kw={"ecolor": INK, "elinewidth": 0.7, "capsize": 2},
            zorder=2,
        )
        if upper_key and model == "astra":
            for i, e in zip(xs, LEVELS_FOR[model], strict=True):
                upper = groups[model, e][upper_key]["mean"]
                peak = max(peak, upper)
                ax.plot(
                    [i + BAR_SHIFTS[model] - 0.075, i + BAR_SHIFTS[model] + 0.075],
                    [upper, upper],
                    color=INK,
                    linewidth=1.2,
                    zorder=4,
                )
        for i, (y, e) in zip(xs, zip(ys, es, strict=True), strict=True):
            ax.annotate(
                label_fmt(y),
                (i + BAR_SHIFTS[model], y + e),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=7.5,
                fontfamily="IBM Plex Mono",
                color=INK,
                rotation=90,
            )
    ax.set_xlim(-0.6, len(LEVELS) - 0.4)
    return peak


def footnotes(text, y, notes):
    for i, note in enumerate(notes):
        text(LEFT, y + 0.24 * i, note.replace("$", "\\$"), 10.5, color=MUTED)


def write_csv(path, groups, fields):
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["model", "effort", "n", *fields])
        for (model, effort), g in groups.items():
            out = [model, effort, g["n"]]
            for f in fields:
                stat = g[f.removesuffix("_se")]
                if f.endswith("_se"):
                    out.append(f"{stat['se']:.4f}" if stat["se"] is not None else "n/a")
                elif isinstance(stat, dict):
                    out.append(f"{stat['mean']:.4f}" if stat["mean"] is not None else "n/a")
                else:
                    out.append(stat)
            writer.writerow(out)


# --- card 1: scores ---------------------------------------------------------------


def score_card(rows, groups, hashes):  # noqa: PLR0915, one linear figure
    fig = new_figure()
    text, line = make_text(fig)
    masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: OpenAI's five models across effort levels",
        "Combined score and runtime at every effort level each model offers, 23 tasks per cell. "
        "Code quality judged by Muse Spark 1.3 and Grok 4.6 under one protocol.",
    )
    chart_top, chart_h = 3.75, 2.85
    for panel, (x0, w) in (("combined", (0.085, 0.405)), ("minutes", (0.565, 0.39))):
        tx = LEFT if panel == "combined" else x0 - 0.04
        text(
            tx,
            3.1,
            "Combined score" if panel == "combined" else "Mean runtime",
            21,
            True,
            heading=True,
        )
        text(
            tx,
            3.42,
            "/100  ·  higher is better  ·  Table 1 carries each model's best level"
            if panel == "combined"
            else "Minutes per task  ·  lower is better",
            12,
            color=MUTED,
        )
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / HEIGHT_IN], facecolor=PAPER)
        style_axis(ax)
        if panel == "combined":
            values = [groups[m, e]["combined"] for m in MODELS for e in LEVELS_FOR[m]]
            lo = 10 * math.floor(min(v["mean"] - (v["se"] or 0) for v in values) / 10)
            hi = math.ceil(max(v["mean"] + (v["se"] or 0) for v in values)) + 3
            ax.set_ylim(lo, hi)
            ax.set_yticks(range(lo, math.floor(hi) + 1, 10 if hi - lo > 40 else 5))
            ax.set_xlim(-0.45, len(LEVELS) - 0.55)
            for model in MODELS:
                xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
                ys = [groups[model, e]["combined"]["mean"] for e in LEVELS_FOR[model]]
                es = [groups[model, e]["combined"]["se"] or 0 for e in LEVELS_FOR[model]]
                ax.plot(xs, ys, color=COLORS[model], linewidth=2, zorder=2)
                ax.errorbar(
                    xs,
                    ys,
                    yerr=es,
                    fmt=MARKERS[model],
                    color=COLORS[model],
                    markersize=7.5,
                    markeredgecolor=INK,
                    markeredgewidth=0.6,
                    ecolor=INK,
                    elinewidth=0.8,
                    capsize=2.5,
                    zorder=3,
                )
        else:
            peak = grouped_bars(ax, groups, "minutes", lambda y: f"{y:.0f}")
            top = 10 * math.ceil(peak / 10) + 10
            ax.set_ylim(0, top)
            ax.set_yticks(range(0, top + 1, 10))

    # Table: each model at its best effort
    best = {
        m: max((groups[m, e] for e in LEVELS_FOR[m]), key=lambda g: g["combined"]["mean"])
        for m in MODELS
    }
    order = sorted(MODELS, key=lambda m: -best[m]["combined"]["mean"])
    text(LEFT, 7.25, "Table 1  |  Each model at its best effort level", 14.5, False, heading=True)
    cols = dict(zip(order, (0.46, 0.58, 0.70, 0.82, 0.94), strict=True))
    line(LEFT, RIGHT, 7.53, INK, 1.2)
    text(LEFT, 7.77, "Measure", 12.5, True)
    for m in order:
        text(cols[m], 7.77, SHORT[m], 12.5, True, ha="right")
        text(cols[m], 8.03, best[m]["effort"].replace("-", " "), 10, ha="right", color=MUTED)
    line(LEFT, RIGHT, 8.2, INK, 0.6)
    table_rows = [
        ("Combined score", "combined", 2, True),
        ("Code quality", "code_quality", 1, False),
        ("    Human readability", "readability", 1, False),
        ("    Maintainability", "maintainability", 1, False),
        ("    Intent recovery", "l2", 1, False),
        ("Tasks passed", "passed", 0, False),
        ("Minutes per task", "minutes", 1, False),
        ("Standard error of combined", "se", 2, False),
    ]
    step, y = 0.32, 8.48
    for label, key, digits, emphasis in table_rows:
        quiet = label.startswith("Standard error")
        text(LEFT, y, label, 13 if emphasis else 12.5, emphasis, color=MUTED if quiet else INK)
        for m in order:
            g = best[m]
            if key == "se":
                shown = f"{g['combined']['se']:.2f}"
            elif key == "passed":
                shown = f"{g['passed']}/{g['n']}"
            else:
                shown = f"{g[key]['mean']:.{digits}f}"
            text(
                cols[m],
                y,
                shown,
                14.5 if emphasis else 13.5,
                emphasis,
                numeric=True,
                ha="right",
                color=MUTED if quiet else INK,
            )
        y += step
    line(LEFT, RIGHT, y - step / 2, INK, 1.2)
    footnotes(
        text,
        y - step / 2 + 0.18,
        [
            "Whiskers are one task standard error. GPT-5.5 offers four effort levels (no max). Combined = 50% functional, "
            "8.5% lint and complexity, 8.5% security, 33% Code quality (24 reviewed panel, 9 intent recovery).",
            "Sol at max is judged on 22 of 23 runs (one judge probe quoted text absent from the code on both attempts). "
            "Terra at max includes a run completed on a second account.",
        ],
    )
    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / "openai-family.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "openai-family-efforts.csv"
    write_csv(
        table,
        groups,
        [
            "combined",
            "combined_se",
            "code_quality",
            "code_quality_se",
            "readability",
            "maintainability",
            "l2",
            "minutes",
            "passed",
            "fallbacks",
        ],
    )
    save(
        out.with_suffix(".json"),
        {
            "rows": len(rows),
            "best_effort": {m: best[m]["effort"] for m in MODELS},
            "table_order": order,
            "expected_rows": {
                f"{m}/{e}": EXPECTED.get((m, e), 23) for m in MODELS for e in LEVELS_FOR[m]
            },
            "weights": {
                "functional": 0.5,
                "quality": 0.085,
                "security": 0.085,
                "code_quality": 0.33,
                "code_quality_split": SPLIT_WITHOUT_L3,
            },
            "sources": hashes,
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
            "visual_qa": "Requires visual inspection after rendering",
        },
    )
    print(out)


# --- card 2: economics ------------------------------------------------------------


def economics_card(rows, groups, hashes, astra_ledger):
    fig = new_figure()
    text, line = make_text(fig)
    masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: OpenAI's five models across effort levels",
        "API-equivalent cost and token use at every effort level; the same runs as the score card, plus Sol's unjudged Max run. "
        "Solver inference only, judging excluded.",
    )
    chart_top, chart_h = 3.75, 2.9
    for panel, (x0, w) in (("usd", (0.085, 0.405)), ("tokens", (0.565, 0.39))):
        tx = LEFT if panel == "usd" else x0 - 0.04
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
            "USD per task at list prices  ·  lower is better  ·  ticks: Astra long-context upper bound"
            if panel == "usd"
            else "Millions of raw tokens, cache reads included  ·  lower is better",
            12,
            color=MUTED,
        )
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / HEIGHT_IN], facecolor=PAPER)
        style_axis(ax)
        if panel == "usd":
            peak = grouped_bars(ax, groups, "usd", lambda y: f"${y:.2f}", upper_key="usd_upper")
        else:
            peak = grouped_bars(ax, groups, "tokens", lambda y: f"{y:.1f}M")
        step = next(s for s in (0.5, 1, 2, 5, 10, 20) if peak / s <= 7)
        top = step * math.ceil(peak / step) + step * 1.2
        ax.set_ylim(0, top)
        ticks = [i * step for i in range(int(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])

    # Table: $/task at every effort, then the sweep total per model
    t0 = 7.2
    text(
        LEFT,
        t0,
        "Table 1  |  Cost per task at each effort level, and the whole sweep",
        14.5,
        False,
        heading=True,
    )
    cols = dict(zip(LEVELS, (0.42, 0.52, 0.62, 0.72, 0.82), strict=True))
    col_total = 0.94
    line(LEFT, RIGHT, t0 + 0.28, INK, 1.2)
    text(LEFT, t0 + 0.52, "Model", 12.5, True)
    for e in LEVELS:
        text(cols[e], t0 + 0.52, e.replace("-", " ").capitalize(), 12.5, True, ha="right")
    text(col_total, t0 + 0.52, "Sweep total", 12.5, True, ha="right")
    text(
        cols["max"],
        t0 + 0.78,
        "API-equivalent USD per task, list prices",
        10,
        ha="right",
        color=MUTED,
    )
    text(col_total, t0 + 0.78, "runs · USD", 10, ha="right", color=MUTED)
    line(LEFT, RIGHT, t0 + 0.95, INK, 0.6)
    step, y = 0.32, t0 + 1.22
    totals = {}
    for m in MODELS:
        rs = [r for r in rows if r["model"] == m]
        totals[m] = {
            "usd": sum(r["usd"] for r in rs),
            "usd_upper": sum(r["usd_upper"] for r in rs),
            "runs": len(rs),
            "tokens": sum(r["tokens"] for r in rs),
            "hours": sum(r["minutes"] for r in rs) / 60,
        }
        text(LEFT, y, NAMES[m], 12.5, True)
        for e in LEVELS:
            shown = f"${groups[m, e]['usd']['mean']:.2f}" if (m, e) in groups else "no level"
            text(
                cols[e],
                y,
                shown,
                13.5,
                numeric=True,
                ha="right",
                color=INK if (m, e) in groups else MUTED,
            )
        text(
            col_total,
            y,
            f"{totals[m]['runs']} · ${totals[m]['usd']:,.2f}",
            13.5,
            numeric=True,
            ha="right",
        )
        y += step
    line(LEFT, RIGHT, y - step / 2, INK, 1.2)
    footnotes(
        text,
        y - step / 2 + 0.18,
        [
            f"List prices checked {astra_ledger['pricing_verified']}, cache-aware, solver inference only; subscription bills differ. "
            "Per million tokens (input / cached input / output):",
            "Sol $4.00 / $0.40 / $20.00; Terra $2.00 / $0.20 / $12.00; Luna $0.20 / $0.02 / $1.20; GPT-5.5 $5.00 / $0.50 / $30.00; "
            "Astra per its report's ledger.",
            f"Astra request sizes are not logged: ticks mark a long-context upper bound (${totals['astra']['usd_upper']:,.0f} for the sweep against "
            f"${totals['astra']['usd']:,.0f} central). Whiskers are one task standard error. Tokens are raw solver totals including cache reads.",
            "Sol's Max cost covers all 23 runs including the one without a published Code quality score. Terra's costs are re-priced "
            "from receipts at the checked list rates, as on its own report.",
        ],
    )
    out = OUTPUT / "openai-family-economics.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "openai-family-economics-efforts.csv"
    write_csv(
        table,
        groups,
        ["usd", "usd_se", "usd_upper", "usd_total", "tokens", "tokens_se", "minutes", "fallbacks"],
    )
    save(
        out.with_suffix(".json"),
        {
            "totals": totals,
            "rates_per_million": {
                m: {"input": r[0], "cached_input": r[1], "output": r[2]} for m, r in RATES.items()
            },
            "astra_rates": astra_ledger["astra_rates_per_million"],
            "pricing_verified": astra_ledger["pricing_verified"],
            "sources": hashes,
            "limitations": astra_ledger["limitations"],
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
        },
    )
    print(out)


def main():
    rows, hashes, astra_ledger = load()
    judged = [r for r in rows]  # every loaded row is a published, judged run
    groups = aggregate(judged)
    score_card(judged, groups, hashes)
    # Economics: add Sol's unjudged Max run so cost covers the whole sweep, as on the Sol economics card.
    sol_ledger = json.loads(LEDGERS["sol"].read_text())
    judged_ids = {r["run_id"] for r in rows}
    extra = [r for r in sol_ledger["rows"] if r["run_id"] not in judged_ids]
    require(len(extra) == 1 and extra[0]["effort"] == "max", f"unexpected Sol rows {len(extra)}")
    manifest = {
        r["run_id"]: r for r in json.loads((RUNS["sol"][0] / "private-manifest.json").read_text())
    }
    for r in extra:
        run = manifest[r["run_id"]]
        receipt = run["solver_receipt"]
        require(
            abs(receipt_cost("sol", receipt) - r["api_equivalent_cost_usd"]) < 1e-5,
            "Sol price drift",
        )
        rows.append(
            {
                "model": "sol",
                "effort": "max",
                "task": r["task"],
                "run_id": r["run_id"],
                "functional": run["functional"],
                "minutes": run["duration_s"] / 60,
                "code_quality": None,
                "l1": None,
                "l2": None,
                "readability": None,
                "maintainability": None,
                "combined": None,
                "usd": r["api_equivalent_cost_usd"],
                "usd_upper": r["api_equivalent_cost_usd"],
                "tokens": receipt["raw_tokens"],
                "fallback": bool(run.get("fallback")),
            }
        )
    EXPECTED.pop(("sol", "max"))
    econ_groups = {}
    for model in MODELS:
        for effort in LEVELS_FOR[model]:
            rs = [r for r in rows if r["model"] == model and r["effort"] == effort]
            require(len(rs) == 23, f"{model} {effort}: {len(rs)} priced rows")
            econ_groups[model, effort] = {
                "model": model,
                "effort": effort,
                "n": len(rs),
                "usd": mean_se(r["usd"] for r in rs),
                "usd_upper": mean_se(r["usd_upper"] for r in rs),
                "usd_total": sum(r["usd"] for r in rs),
                "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
                "minutes": mean_se(r["minutes"] for r in rs),
                "fallbacks": sum(r["fallback"] for r in rs),
            }
    economics_card(rows, econ_groups, hashes, astra_ledger)


if __name__ == "__main__":
    main()

"""GPT-6 Astra, Claude Fable 5.1 and Muse Spark 1.3 on VulcanBench Frontier v4: two cards.

Same layout as the OpenAI family cards. Astra and Fable rows come from the
frozen Code quality v3.4 summary, manifest and cost ledger that published
them. Muse Spark 1.3 rows come from the Contributor-tier Muse Code sweep
(runs-muse13-contributor-cii-v4-v2) with Code quality from the v3.12 summary
(Claude Opus 5 alone, 99 of 115 runs judged). The score card plots the
functional score all three share on every run and carries the combined score
in its table. Nothing is re-judged.

    python scripts/cii-v4-board/make_frontier_trio_cards.py

Writes docs/results/swe-v4-muse-astra-fable-2026-09/frontier-trio.png (score)
and frontier-trio-economics.png (cost and tokens), each with an SVG, a CSV of
the plotted aggregates and a JSON record of sources and hashes.
"""

from __future__ import annotations

import csv
import importlib.util
import json
import math
import statistics
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.evaluator.reviewed_score import WEIGHTS_V3  # noqa: E402
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    "openai_family_cards", Path(__file__).with_name("make_openai_family_cards.py")
)
family = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(family)
require, mean_se, yf = family.require, family.mean_se, family.yf
PAPER, INK, RULE, MUTED = family.PAPER, family.INK, family.RULE, family.MUTED
LEFT, RIGHT, WIDTH_IN, HEIGHT_IN = family.LEFT, family.RIGHT, family.WIDTH_IN, family.HEIGHT_IN

OUTPUT = ROOT / "docs/results/swe-v4-muse-astra-fable-2026-09"
JUDGED = ROOT / "runs-code-quality-maintenance-v3.4"
LEDGER = ROOT / "docs/results/swe-v4-astra-fable51-2026-09/api-equivalent-costs.json"
MUSE_ROOT = ROOT / "runs-muse13-contributor-cii-v4-v2"
MUSE_JUDGED = ROOT / "runs-code-quality-maintenance-v3.12"  # Claude Opus 5 alone
MUSE_SPEC = "muse-code:muse-spark-1.3-contributor"
MODELS = ("astra", "fable", "muse")
NAMES = {"astra": "GPT-6 Astra", "fable": "Claude Fable 5.1", "muse": "Muse Spark 1.3"}
SHORT = {"astra": "Astra", "fable": "Fable 5.1", "muse": "Muse 1.3"}
COLORS = {"astra": "#10A37F", "fable": "#D97757", "muse": "#0866ff"}
MARKERS = {"astra": "o", "fable": "s", "muse": "D"}
LEVELS_FOR = {m: tuple(LEVELS) for m in MODELS}
LEVELS_FOR["muse"] = ("low", "medium", "high", "extra-high")  # Contributor tier has no max
BAR_SHIFTS = {"astra": -0.26, "fable": 0.0, "muse": 0.26}
# USD per million tokens (input, cached input, output), Meta pricing page checked 2026-09-06.
MUSE_CONTRIBUTOR = (0.10, 0.002, 0.20)
MUSE_STANDARD = (1.25, 0.15, 4.25)
CAP_S = 10800  # flat v4 task timeout since 2026-09-13 (docs/DECISIONS.md)


def muse_price(tokens, rates):
    cached = min(tokens["cached_input"], tokens["prompt"])
    inp, cch, out = rates
    return ((tokens["prompt"] - cached) * inp + cached * cch + tokens["completion"] * out) / 1e6


def load_judged():
    """Astra and Fable: published v3.4 rows with their ledger costs."""
    summary = json.loads((JUDGED / "summary.json").read_text())
    protocol = json.loads((JUDGED / "protocol.json").read_text())
    require(summary["protocol"] == "code-quality-maintenance-v3.4", "wrong protocol")
    require(summary["ready_for_publication"], "v3.4 summary not final")
    require(set(summary["passing_panels"]) == set(family.PANELS), "v3.4 panels")
    require(
        protocol["weights"]["functional"] == WEIGHTS_V3["functional"]
        and protocol["weights"]["code_quality"]["total"] == WEIGHTS_V3["human_like"],
        "weights drift",
    )
    manifest = {r["id"]: r for r in json.loads((JUDGED / "private-manifest.json").read_text())}
    ledger = json.loads(LEDGER.read_text())
    costs = {
        (r["model"], r["run_id"]): (r["estimated_usd"], r.get("long_context_upper_usd"))
        for r in ledger["rows"]
    }
    rows = []
    for entry in summary["rows"]:
        if entry["model"] not in ("astra", "fable") or not entry.get("published"):
            continue
        run = manifest[entry["id"]]
        cq, _, _ = family.code_quality(entry)
        receipt = run["solver_receipt"]
        require(type(receipt.get("raw_tokens")) is int, f"no receipt for {run['run_id']}")
        usd, upper = costs[entry["model"], run["run_id"]]
        rows.append(
            {
                "model": entry["model"],
                "effort": entry["effort"],
                "task": entry["task"],
                "run_id": run["run_id"],
                "functional": run["functional"],
                "minutes": run["duration_s"] / 60,
                "code_quality": cq,
                "combined": family.composite(run, cq, WEIGHTS_V3["human_like"]),
                "usd": usd,
                "usd_upper": upper if upper is not None else usd,
                "tokens": receipt["raw_tokens"],
            }
        )
    hashes = {
        "summary": digest((JUDGED / "summary.json").read_bytes()),
        "protocol": digest((JUDGED / "protocol.json").read_bytes()),
        "ledger": digest(LEDGER.read_bytes()),
    }
    return rows, hashes, ledger


def load_muse_judged():
    """Muse's Code quality from v3.12 (Claude Opus 5 alone), keyed by run id."""
    summary = json.loads((MUSE_JUDGED / "summary.json").read_text())
    require(summary["protocol"] == "code-quality-maintenance-v3.12", "wrong Muse protocol")
    require(
        summary["ready_for_publication"] and summary["passing_panels"] == ["claude"],
        "v3.12 summary",
    )
    manifest = {r["id"]: r for r in json.loads((MUSE_JUDGED / "private-manifest.json").read_text())}
    judged = {}
    for entry in summary["rows"]:
        if entry.get("published"):
            pub = entry["published"]
            judged[manifest[entry["id"]]["run_id"]] = (pub["code_quality"], pub["composite_v3"])
    hashes = {
        "summary": digest((MUSE_JUDGED / "summary.json").read_bytes()),
        "protocol": digest((MUSE_JUDGED / "protocol.json").read_bytes()),
    }
    return judged, hashes


def load_muse():
    """Muse Spark 1.3: scored sweep summaries, priced from their own token receipts."""
    judged, judged_hashes = load_muse_judged()
    protocol = json.loads((MUSE_ROOT / "protocol.json").read_text())
    status = json.loads((MUSE_ROOT / "status.json").read_text())
    require(status["state"] == "complete", "Muse sweep not complete")
    require(protocol["model"] == MUSE_SPEC, "Muse protocol model")
    rows, aborted = [], {}
    for effort in ("minimal", *LEVELS_FOR["muse"]):
        attempts = [d for d in (MUSE_ROOT / effort).iterdir() if d.is_dir()]
        scored = [d for d in attempts if (d / "summary.json").exists()]
        aborted[effort] = len(attempts) - len(scored)
        for run_dir in scored:
            s = json.loads((run_dir / "summary.json").read_text())
            require(
                s["model"] == MUSE_SPEC and s["task_hash"] == protocol["task_hashes"][s["task_id"]],
                f"{run_dir.name}: model or task drift",
            )
            usd = muse_price(s["tokens"], MUSE_CONTRIBUTOR)
            require(abs(usd - s["cost_usd"]) < 1e-4, f"price drift on {s['run_id']}")
            cq, combined = judged.get(s["run_id"], (None, None))
            rows.append(
                {
                    "model": "muse",
                    "effort": effort,
                    "task": s["task_id"],
                    "run_id": s["run_id"],
                    "functional": s["scores"]["functional"],
                    "minutes": s["duration_s"] / 60,
                    "seconds": s["duration_s"],
                    "code_quality": cq,
                    "combined": combined,
                    "usd": usd,
                    "usd_upper": muse_price(s["tokens"], MUSE_STANDARD),
                    "tokens": s["total_tokens"],
                }
            )
    require(
        len(judged) == 99 and sum(r["combined"] is not None for r in rows) == 99, "Muse judged rows"
    )
    hashes = {
        "protocol": digest((MUSE_ROOT / "protocol.json").read_bytes()),
        "results": digest((MUSE_ROOT / "results.jsonl").read_bytes()),
        "code_quality_v3.12": judged_hashes,
    }
    return rows, hashes, aborted, protocol


def aggregate(rows, efforts_for):
    groups = {}
    for model in MODELS:
        for effort in efforts_for[model]:
            rs = [r for r in rows if r["model"] == model and r["effort"] == effort]
            require(len(rs) == 23, f"{model} {effort}: {len(rs)} rows")
            judged = [r for r in rs if r["combined"] is not None]
            groups[model, effort] = {
                "model": model,
                "effort": effort,
                "n": len(rs),
                "functional": mean_se(100 * r["functional"] for r in rs),
                "combined": mean_se(r["combined"] for r in judged),
                "code_quality": mean_se(r["code_quality"] for r in judged),
                "minutes": mean_se(r["minutes"] for r in rs),
                "usd": mean_se(r["usd"] for r in rs),
                "usd_upper": mean_se(r["usd_upper"] for r in rs),
                "usd_total": sum(r["usd"] for r in rs),
                "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
                "passed": sum(r["functional"] == 1 for r in rs),
            }
    return groups


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
    for model, x in zip(MODELS, (LEFT, 0.245, 0.46), strict=True):
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


def grouped_bars(ax, groups, key, label_fmt, upper_key=None):
    peak = max(g[key]["mean"] + (g[key]["se"] or 0) for g in groups.values())
    for model in MODELS:
        xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
        ys = [groups[model, e][key]["mean"] for e in LEVELS_FOR[model]]
        es = [groups[model, e][key]["se"] or 0 for e in LEVELS_FOR[model]]
        tops = [y + e for y, e in zip(ys, es, strict=True)]
        ax.bar(
            [i + BAR_SHIFTS[model] for i in xs],
            ys,
            width=0.24,
            color=COLORS[model],
            edgecolor=INK,
            linewidth=0.4,
            yerr=es,
            error_kw={"ecolor": INK, "elinewidth": 0.7, "capsize": 2},
            zorder=2,
        )
        if upper_key and model != "fable":
            for i, e in zip(xs, LEVELS_FOR[model], strict=True):
                upper = groups[model, e][upper_key]["mean"]
                # A tick that would cut through the bar's label lifts the label above it.
                slot = xs.index(i)
                if upper - tops[slot] < 0.14 * peak:
                    tops[slot] = upper
                ax.plot(
                    [i + BAR_SHIFTS[model] - 0.12, i + BAR_SHIFTS[model] + 0.12],
                    [upper, upper],
                    color=INK,
                    linewidth=1.2,
                    zorder=4,
                )
        for i, y, label_y in zip(xs, ys, tops, strict=True):
            ax.annotate(
                label_fmt(y),
                (i + BAR_SHIFTS[model], label_y),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8.5,
                fontfamily="IBM Plex Mono",
                color=INK,
                rotation=90,
            )
    ax.set_xlim(-0.6, len(LEVELS) - 0.4)
    return peak


def write_csv(path, groups, fields):
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["model", "effort", "n", *fields])
        for (model, effort), g in groups.items():
            out = [model, effort, g["n"]]
            for f in fields:
                stat = g[f.removesuffix("_se")]
                if isinstance(stat, dict):
                    value = stat["se" if f.endswith("_se") else "mean"]
                    out.append(f"{value:.4f}" if value is not None else "not judged")
                else:
                    out.append(stat)
            writer.writerow(out)


def panel_titles(text, x0, left_panel, title, caption):
    tx = LEFT if left_panel else x0 - 0.04
    text(tx, 3.1, title, 21, True, heading=True)
    text(tx, 3.42, caption, 12, color=MUTED)


def score_card(groups, minimal, aborted, capped_low, hashes):  # noqa: PLR0915, one linear figure
    fig = family.new_figure()
    text, line = family.make_text(fig)
    masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: Astra, Fable 5.1 and Muse Spark 1.3",
        "Functional score and runtime at every effort level each model offers, 23 tasks per cell, "
        "one task at a time. Same tasks and hidden tests for all three.",
    )
    chart_top, chart_h = 3.75, 2.85
    ax = fig.add_axes([0.085, yf(chart_top + chart_h), 0.405, chart_h / HEIGHT_IN], facecolor=PAPER)
    panel_titles(
        text,
        0.085,
        True,
        "Functional score",
        "/100, share of hidden tests passed  ·  higher is better  ·  a regression scores 0",
    )
    family.style_axis(ax)
    values = [groups[m, e]["functional"] for m in MODELS for e in LEVELS_FOR[m]]
    lo = 10 * math.floor(min(v["mean"] - v["se"] for v in values) / 10)
    ax.set_ylim(lo, 104)
    ax.set_yticks(range(lo, 101, 10))
    ax.set_xlim(-0.45, len(LEVELS) - 0.55)
    for model in MODELS:
        xs = [LEVELS.index(e) for e in LEVELS_FOR[model]]
        ys = [groups[model, e]["functional"]["mean"] for e in LEVELS_FOR[model]]
        es = [groups[model, e]["functional"]["se"] for e in LEVELS_FOR[model]]
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
    ax = fig.add_axes([0.565, yf(chart_top + chart_h), 0.39, chart_h / HEIGHT_IN], facecolor=PAPER)
    panel_titles(text, 0.565, False, "Mean runtime", "Minutes per task  ·  lower is better")
    family.style_axis(ax)
    peak = grouped_bars(ax, groups, "minutes", lambda y: f"{y:.0f}")
    top = 20 * math.ceil(peak / 20) + 20
    ax.set_ylim(0, top)
    ax.set_yticks(range(0, top + 1, 20))

    best = {
        m: max(
            (groups[m, e] for e in LEVELS_FOR[m]),
            key=lambda g: (g["functional"]["mean"], -g["minutes"]["mean"]),
        )
        for m in MODELS
    }
    order = sorted(MODELS, key=lambda m: -best[m]["functional"]["mean"])
    text(
        LEFT,
        7.25,
        "Table 1  |  Each model at its best effort level by functional score",
        14.5,
        False,
        heading=True,
    )
    cols = dict(zip(order, (0.58, 0.76, 0.94), strict=True))
    line(LEFT, RIGHT, 7.53, INK, 1.2)
    text(LEFT, 7.77, "Measure", 12.5, True)
    for m in order:
        text(cols[m], 7.77, SHORT[m], 12.5, True, ha="right")
        text(cols[m], 8.03, best[m]["effort"].replace("-", " "), 10, ha="right", color=MUTED)
    line(LEFT, RIGHT, 8.2, INK, 0.6)
    table_rows = [
        ("Functional score", "functional", 2, True),
        ("Tasks fully passed", "passed", 0, False),
        ("Minutes per task", "minutes", 1, False),
        ("Combined score (with Code quality)", "combined", 2, False),
        ("Code quality", "code_quality", 1, False),
        ("Standard error of functional", "se", 2, False),
    ]
    step, y = 0.32, 8.48
    for label, key, digits, emphasis in table_rows:
        quiet = label.startswith("Standard error")
        text(LEFT, y, label, 13 if emphasis else 12.5, emphasis, color=MUTED if quiet else INK)
        for m in order:
            g = best[m]
            if key == "se":
                shown = f"{g['functional']['se']:.2f}"
            elif key == "passed":
                shown = f"{g['passed']}/{g['n']}"
            elif g[key]["mean"] is None:
                shown = "not judged"
            else:
                shown = f"{g[key]['mean']:.{digits}f}"
            missing = shown == "not judged"
            text(
                cols[m],
                y,
                shown,
                14.5 if emphasis else 13.5,
                emphasis,
                numeric=True,
                ha="right",
                color=MUTED if quiet or missing else INK,
            )
        y += step
    line(LEFT, RIGHT, y - step / 2, INK, 1.2)
    total_aborted = sum(aborted[e] for e in LEVELS_FOR["muse"])
    family.footnotes(
        text,
        y - step / 2 + 0.18,
        [
            "Whiskers are one task standard error. Harnesses: Codex (Astra), Claude Code (Fable 5.1), Muse Code 1.0.3 on Meta's "
            "Contributor tier (Muse Spark 1.3), which offers no max level.",
            "Code quality: Astra and Fable from v3.4, two judges (Muse Spark 1.3 and Grok 4.6). Muse from v3.12, Claude Opus 5 alone, "
            "on 99 of 115 runs (14 unfinished, one no source change, one unreconstructible patch).",
            f"Muse's minimal level is off this axis ({minimal['functional']['mean']:.1f} functional, "
            f"{minimal['minutes']['mean']:.0f} minutes per task). Muse ran Low's first 17 tasks under the former 10-hour bound; "
            f"one outlasted today's 3-hour bound, and with it scored 0 Muse at Low is {capped_low:.1f}.",
            f"Muse attempts ended by a provider stream error before any patch ({total_aborted} at these levels) were rerun "
            "from a clean workspace; timeouts and failing patches are scored as they stand.",
        ],
    )
    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / "frontier-trio.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "frontier-trio-efforts.csv"
    write_csv(
        table,
        {**groups, ("muse", "minimal"): minimal},
        [
            "functional",
            "functional_se",
            "passed",
            "minutes",
            "minutes_se",
            "combined",
            "code_quality",
        ],
    )
    save(
        out.with_suffix(".json"),
        {
            "best_effort": {m: best[m]["effort"] for m in MODELS},
            "table_order": order,
            "muse_low_functional_under_3h_bound": capped_low,
            "muse_provider_aborted_attempts": aborted,
            "muse_code_quality": "v3.12, Claude Opus 5 alone, 99 of 115 runs",
            "sources": hashes,
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
            "visual_qa": "Requires visual inspection after rendering",
        },
    )
    print(out)


def economics_card(rows, groups, minimal, hashes, ledger):
    fig = family.new_figure()
    text, line = family.make_text(fig)
    masthead(
        fig,
        text,
        line,
        "VulcanBench Frontier v4: Astra, Fable 5.1 and Muse Spark 1.3",
        "API-equivalent cost and token use at every effort level; the same runs as the score card. "
        "Solver inference only. All three sweeps ran on subscriptions.",
    )
    chart_top, chart_h = 3.75, 2.9
    for panel, (x0, w) in (("usd", (0.085, 0.405)), ("tokens", (0.565, 0.39))):
        panel_titles(
            text,
            x0,
            panel == "usd",
            "API-equivalent cost" if panel == "usd" else "Tokens per task",
            "USD per task at list prices  ·  lower is better  ·  ticks: upper bounds, see notes"
            if panel == "usd"
            else "Millions of raw tokens, cache reads included  ·  lower is better",
        )
        ax = fig.add_axes([x0, yf(chart_top + chart_h), w, chart_h / HEIGHT_IN], facecolor=PAPER)
        family.style_axis(ax)
        if panel == "usd":
            peak = grouped_bars(ax, groups, "usd", lambda y: f"${y:.2f}", upper_key="usd_upper")
        else:
            peak = grouped_bars(ax, groups, "tokens", lambda y: f"{y:.1f}M")
        step = next(s for s in (0.5, 1, 2, 5, 10, 20, 50) if peak / s <= 7)
        top = step * math.ceil(peak / step) + step * 1.2
        ax.set_ylim(0, top)
        ticks = [i * step for i in range(int(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])

    t0 = 7.2
    text(
        LEFT,
        t0,
        "Table 1  |  Cost per task at each effort level, and Low to Max in total",
        14.5,
        False,
        heading=True,
    )
    cols = dict(zip(LEVELS, (0.34, 0.43, 0.52, 0.61, 0.70), strict=True))
    col_total = 0.94
    line(LEFT, RIGHT, t0 + 0.28, INK, 1.2)
    text(LEFT, t0 + 0.52, "Model", 12.5, True)
    for e in LEVELS:
        text(cols[e], t0 + 0.52, e.replace("-", " ").capitalize(), 12.5, True, ha="right")
    text(col_total, t0 + 0.52, "Low to Max total", 12.5, True, ha="right")
    text(
        cols["max"],
        t0 + 0.78,
        "API-equivalent USD per task, list prices",
        10,
        ha="right",
        color=MUTED,
    )
    text(col_total, t0 + 0.78, "runs · USD · tokens", 10, ha="right", color=MUTED)
    line(LEFT, RIGHT, t0 + 0.95, INK, 0.6)
    step, y = 0.32, t0 + 1.22
    totals = {}
    for m in MODELS:
        rs = [r for r in rows if r["model"] == m and r["effort"] in LEVELS_FOR[m]]
        totals[m] = {
            "usd": sum(r["usd"] for r in rs),
            "usd_upper": sum(r["usd_upper"] for r in rs),
            "runs": len(rs),
            "tokens": sum(r["tokens"] for r in rs),
            "hours": sum(r["minutes"] for r in rs) / 60,
        }
        text(LEFT, y, NAMES[m], 12.5, True)
        for e in LEVELS:
            present = (m, e) in groups
            text(
                cols[e],
                y,
                f"${groups[m, e]['usd']['mean']:.2f}" if present else "no level",
                13.5,
                numeric=True,
                ha="right",
                color=INK if present else MUTED,
            )
        tokens = totals[m]["tokens"]
        shown = f"{tokens / 1e9:.2f}B" if tokens >= 1e9 else f"{tokens / 1e6:.0f}M"
        text(
            col_total,
            y,
            f"{totals[m]['runs']} · ${totals[m]['usd']:,.2f} · {shown}",
            13.5,
            numeric=True,
            ha="right",
        )
        y += step
    line(LEFT, RIGHT, y - step / 2, INK, 1.2)
    fable = ledger["claude_rates_per_million"]["claude-fable-5-1"]
    astra = ledger["astra_rates_per_million"]
    family.footnotes(
        text,
        y - step / 2 + 0.18,
        [
            "List prices, cache-aware, solver inference only; subscription bills differ. Per million tokens: "
            f"Astra ${astra['input']:g} input / ${astra['cache_read']:g} cached / ${astra['output']:g} output; "
            f"Fable 5.1 ${fable[0]:g} / ${fable[1]:g} / ${fable[4]:g} plus cache writes at ${fable[2]:g} to ${fable[3]:g};",
            f"Muse Spark 1.3 on Meta's Contributor tier ${MUSE_CONTRIBUTOR[0]:.2f} / ${MUSE_CONTRIBUTOR[1]:.3f} / "
            f"${MUSE_CONTRIBUTOR[2]:.2f}, which lets Meta train on the prompts and completions. "
            f"Rates checked {ledger['pricing_verified']}.",
            f"Muse ticks re-price the same tokens at Meta's Standard tier (${MUSE_STANDARD[0]:.2f} / ${MUSE_STANDARD[1]:.2f} / "
            f"${MUSE_STANDARD[2]:.2f}): ${totals['muse']['usd_upper']:,.0f} for these four levels against "
            f"${totals['muse']['usd']:,.0f}. Astra ticks mark its long-context upper bound "
            f"(${totals['astra']['usd_upper']:,.0f} against ${totals['astra']['usd']:,.0f}).",
            f"Muse's minimal level is off this axis: ${minimal['usd']['mean']:.2f} and {minimal['tokens']['mean']:.1f}M tokens per task. "
            "Whiskers are one task standard error. Tokens are raw solver totals including cache reads.",
        ],
    )
    out = OUTPUT / "frontier-trio-economics.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "frontier-trio-economics-efforts.csv"
    write_csv(
        table,
        {**groups, ("muse", "minimal"): minimal},
        ["usd", "usd_se", "usd_upper", "usd_total", "tokens", "tokens_se", "minutes"],
    )
    save(
        out.with_suffix(".json"),
        {
            "totals_low_to_max": totals,
            "muse_rates_per_million": {
                "contributor": MUSE_CONTRIBUTOR,
                "standard_upper_tick": MUSE_STANDARD,
            },
            "astra_rates": astra,
            "fable_rates": fable,
            "pricing_verified": ledger["pricing_verified"],
            "sources": hashes,
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
        },
    )
    print(out)


def main():
    judged, judged_hashes, ledger = load_judged()
    muse, muse_hashes, aborted, _ = load_muse()
    rows = judged + muse
    groups = aggregate(rows, LEVELS_FOR)
    minimal = aggregate(muse, {"astra": (), "fable": (), "muse": ("minimal",)})["muse", "minimal"]
    low = [r for r in muse if r["effort"] == "low"]
    capped_low = statistics.mean(
        0.0 if r["seconds"] > CAP_S + 60 else 100 * r["functional"] for r in low
    )
    hashes = {"code_quality_v3.4": judged_hashes, "muse_sweep": muse_hashes}
    score_card(groups, minimal, aborted, capped_low, hashes)
    economics_card(rows, groups, minimal, hashes, ledger)


if __name__ == "__main__":
    main()

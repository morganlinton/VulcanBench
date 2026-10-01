"""GPT-6.1 Sol, GPT-6 Sol and GPT-5.6 Sol on VulcanBench Frontier v4: two cards.

- sol-family-results: tasks passed and the share of documented behaviors fixed
  (hidden tests), with minutes per task.
- sol-family-economics: API-equivalent cost and raw tokens per task.

Both read the population records the per-model releases froze (every run,
all 23 tasks at each of five levels) and the run summaries they point at. No
Code quality enters these cards: GPT-6.1 Sol's judging (protocol v3.18) was
still running when they were first drawn, so the cards say so and stay on
measures that are already final. Every cost is recomputed from the run's
Codex receipt at the model's list price and must match the record's stamp.

    python scripts/cii-v4-board/make_sol_family_cards.py
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
from matplotlib import font_manager, ticker
from matplotlib.patches import FancyBboxPatch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

OUTPUT = ROOT / "docs/results/swe-v4-sol-family-2026-09"
RESULTS = ROOT / "docs/results"
# key: (population record, key in it, list rates in/cached/out per million, Codex CLI, pricing checked)
SOURCES = {
    "gpt61sol": (
        "swe-v4-gpt61-sol-2026-09/comparison.json",
        "gpt61sol",
        (2.00, 0.10, 10.00),
        "Codex 0.159.0",
        "2026-09-29",
    ),
    "gpt6sol": (
        "swe-v4-gpt6-sol-2026-09/comparison.json",
        "gpt6sol",
        (2.00, 0.20, 10.00),
        "Codex 0.155.0",
        "2026-09-25",
    ),
    "sol56": (
        "swe-v4-sol-2026-09/comparison.json",
        "sol",
        (4.00, 0.40, 20.00),
        "Codex",
        "2026-09-11",
    ),
}
MODELS = ("gpt61sol", "gpt6sol", "sol56")
NAMES = {"gpt61sol": "GPT-6.1 Sol", "gpt6sol": "GPT-6 Sol", "sol56": "GPT-5.6 Sol"}
COLORS = {"gpt61sol": "#0B3D2E", "gpt6sol": "#10A37F", "sol56": "#7A9A2E"}
MARKERS = {"gpt61sol": "o", "gpt6sol": "D", "sol56": "s"}
HOLLOW = {"sol56"}
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
WIDTH_IN, HEIGHT_IN = 16, 13.45


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


def load(model):
    ledger_name, key, rates, _, _ = SOURCES[model]
    path = RESULTS / ledger_name
    record = json.loads(path.read_text())
    require(not record.get("excluded") and not record.get("missing"), f"{model}: incomplete record")
    runs = []
    for r in record["rows"]:
        if r["model"] != key:
            continue
        usage = r["solver_receipt"]["usage"]
        cached = min(usage["cached_input_tokens"], usage["input_tokens"])
        usd = (
            (usage["input_tokens"] - cached) * rates[0]
            + cached * rates[1]
            + usage["output_tokens"] * rates[2]
        ) / 1e6
        require(abs(usd - r["api_equivalent_cost_usd"]) < 1e-5, f"price drift on {r['run_id']}")
        summary = json.loads((Path(r["source_directory"]) / "summary.json").read_text())
        f2p = summary["verifier"].get("fail_to_pass") or {}
        runs.append(
            {
                "effort": r["effort"],
                "passed": r["functional"] == 1,
                "functional": 100 * r["functional"],
                "fixed": sum(bool(v) for v in f2p.values()),
                "behaviors": len(f2p),
                "minutes": r["duration_s"] / 60,
                "usd": usd,
                "tokens": r["solver_receipt"]["raw_tokens"] / 1e6,
            }
        )
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        require(len(rs) == 23, f"{model} {effort}: {len(rs)} runs")
        groups[effort] = {
            "passed": sum(r["passed"] for r in rs),
            "functional": mean_se(r["functional"] for r in rs),
            "fixed": sum(r["fixed"] for r in rs),
            "behaviors": sum(r["behaviors"] for r in rs),
            "minutes": mean_se(r["minutes"] for r in rs),
            "usd": mean_se(r["usd"] for r in rs),
            "usd_total": sum(r["usd"] for r in rs),
            "tokens": mean_se(r["tokens"] for r in rs),
        }
    return groups, digest(path.read_bytes())


def figure():
    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    fig = plt.figure(figsize=(WIDTH_IN, HEIGHT_IN), dpi=150, facecolor=PAPER)

    def yf(inches):
        return 1 - inches / HEIGHT_IN

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

    return fig, yf, text, line


def marker_style(model, size=8):
    hollow = model in HOLLOW
    return {
        "marker": MARKERS[model],
        "markersize": size,
        "markerfacecolor": PAPER if hollow else COLORS[model],
        "markeredgecolor": COLORS[model] if hollow else INK,
        "markeredgewidth": 1.4 if hollow else 0.6,
    }


def masthead(fig, yf, text, line, title, subtitle):
    left, right = 0.06, 0.94
    logo = fig.add_axes([left, yf(0.86), 0.42 / WIDTH_IN, 0.42 / HEIGHT_IN])
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
    text(left, 1.78, title, 33, True, heading=True)
    text(left, 2.22, subtitle, 15, color=MUTED)
    step = (right - left) / len(MODELS)
    for i, model in enumerate(MODELS):
        x = left + i * step
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
        text(x + 0.018, 2.68, f"{NAMES[model]}  ·  {SOURCES[model][3]}", 14, True)


def chart(fig, yf, text, x0, w, title, sub, groups, key, fmt, log=False, ylim=None):
    tx = 0.06 if x0 < 0.5 else x0 - 0.06
    text(tx, 3.1, title, 21, True, heading=True)
    text(tx, 3.42, sub, 12, color=MUTED)
    ax = fig.add_axes([x0, yf(3.75 + 3.0), w, 3.0 / HEIGHT_IN], facecolor=PAPER)
    xs = list(range(len(LEVELS)))
    ax.set_xticks(xs, [e.replace("-", " ").capitalize() for e in LEVELS])
    ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
    ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
    ax.spines[["top", "right"]].set_visible(False)
    ax.spines[["left", "bottom"]].set_color(RULE)
    ax.grid(axis="y", color=RULE, linewidth=0.5)
    ax.set_axisbelow(True)
    ax.set_xlim(-0.45, len(LEVELS) - 0.55)
    offsets = {m: (i - 1) * 0.08 for i, m in enumerate(MODELS)}
    for model in MODELS:
        g = groups[model]
        px = [x + offsets[model] for x in xs]
        vals = [g[e][key] for e in LEVELS]
        ys = [v["mean"] if isinstance(v, dict) else v for v in vals]
        es = [v["se"] if isinstance(v, dict) else 0 for v in vals]
        ax.plot(px, ys, color=COLORS[model], linewidth=2, zorder=2)
        if any(es):
            ax.errorbar(
                px, ys, yerr=es, fmt="none", ecolor=INK, elinewidth=0.8, capsize=2.5, zorder=3
            )
        ax.plot(px, ys, linestyle="none", zorder=4, color=COLORS[model], **marker_style(model))
    if ylim:
        ax.set_ylim(*ylim)
    if log:
        ax.set_yscale("log")
        allv = [groups[m][e][key]["mean"] for m in MODELS for e in LEVELS]
        ticks = [
            t for t in (0.3, 0.5, 1, 2, 3, 5, 10, 20) if min(allv) / 1.6 <= t <= max(allv) * 1.6
        ]
        ax.yaxis.set_major_locator(ticker.FixedLocator(ticks))
        ax.yaxis.set_minor_locator(ticker.NullLocator())
    ax.yaxis.set_major_formatter(ticker.FuncFormatter(fmt))
    return ax


def table(text, line, groups, t0, title, measures):
    left, right = 0.06, 0.94
    cols = {e: x for e, x in zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True)}
    text(left, t0, title, 14.5, False, heading=True)
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Model and measure", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], t0 + 0.52, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, t0 + 0.7, INK, 0.6)
    y, step = t0 + 0.98, 0.33
    for model in MODELS:
        text(left, y, NAMES[model], 13, True)
        for i, (label, fmt) in enumerate(measures):
            if i:
                text(left + 0.02, y, label, 12)
            else:
                text(left + 0.16, y, label, 12, color=MUTED)
            for effort in LEVELS:
                text(cols[effort], y, fmt(groups[model][effort]), 13, numeric=True, ha="right")
            y += step
        line(left, right, y - step / 2, RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)
    return y - step / 2


def notes(text, y, lines):
    for i, note in enumerate(lines):
        text(0.06, y + 0.25 + 0.26 * i, note, 10.5, color=MUTED)


def write(fig, name, groups, hashes, fields):
    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / f"{name}.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table_path = OUTPUT / f"{name}-efforts.csv"
    with table_path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["model", "effort", *fields])
        for model in MODELS:
            for effort in LEVELS:
                g = groups[model][effort]
                writer.writerow(
                    [model, effort]
                    + [f"{g[f]['mean']:.6f}" if isinstance(g[f], dict) else g[f] for f in fields]
                )
    save(
        out.with_suffix(".json"),
        {
            "card": name,
            "sources": {
                m: {
                    "record": SOURCES[m][0],
                    "record_sha256": hashes[m],
                    "list_rates_per_million": SOURCES[m][2],
                    "pricing_verified": SOURCES[m][4],
                }
                for m in MODELS
            },
            "code_quality": "not included; GPT-6.1 Sol judging (v3.18) in progress when drawn",
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table_path.name,
        },
    )
    print(out)


COMMON = [
    "Each model ran the same 23 tasks once at each effort level through Codex on a ChatGPT Pro plan; no run reached the 3-hour bound.",
    "GPT-6.1 Sol ran on Codex CLI 0.159.0 and GPT-6 Sol on 0.155.0 (the first releases that serve them); GPT-5.6 Sol on 0.153.4 or older.",
]


def main():
    groups, hashes = {}, {}
    for model in MODELS:
        groups[model], hashes[model] = load(model)

    fig, yf, text, line = figure()
    masthead(
        fig,
        yf,
        text,
        line,
        "VulcanBench Frontier v4: three generations of Sol",
        "Hidden-test results at every effort level, 23 tasks per effort. Code quality for GPT-6.1 Sol is still being judged.",
    )
    chart(
        fig,
        yf,
        text,
        0.085,
        0.405,
        "Tasks passed",
        "of 23  ·  every hidden test green  ·  higher is better",
        groups,
        "passed",
        lambda v, _: f"{v:g}",
        ylim=(0, 24),
    )
    chart(
        fig,
        yf,
        text,
        0.585,
        0.37,
        "Behaviors fixed",
        "mean share of each task's documented behaviors, %  ·  higher is better",
        groups,
        "functional",
        lambda v, _: f"{v:g}",
    )
    y = table(
        text,
        line,
        groups,
        7.45,
        "Table 1  |  Each model at each effort level",
        [
            ("Tasks passed, of 23", lambda g: f"{g['passed']}"),
            ("Behaviors fixed", lambda g: f"{g['fixed']} of {g['behaviors']}"),
            ("Minutes per task", lambda g: f"{g['minutes']['mean']:.1f}"),
        ],
    )
    notes(
        text,
        y,
        [
            "Functional results only. The published combined score adds 33% Code quality from two judges; GPT-6.1 Sol's judging (v3.18) is in",
            "progress, so this card uses no Code quality for any model. Whiskers are one task standard error. GPT-6 Sol's and GPT-6.1",
            "Sol's sweeps each overlapped another model's Code quality judging on the same machine, which can lengthen their minutes.",
            *COMMON,
        ],
    )
    write(
        fig,
        "sol-family-results",
        groups,
        hashes,
        ["passed", "fixed", "behaviors", "functional", "minutes"],
    )

    fig, yf, text, line = figure()
    masthead(
        fig,
        yf,
        text,
        line,
        "VulcanBench Frontier v4: three generations of Sol",
        "API-equivalent cost and token use at every effort level, 23 tasks per effort; solver inference only.",
    )
    chart(
        fig,
        yf,
        text,
        0.085,
        0.405,
        "API-equivalent cost",
        "USD per task at list prices  ·  log scale  ·  lower is better",
        groups,
        "usd",
        lambda v, _: f"\\${v:g}",
        log=True,
    )
    chart(
        fig,
        yf,
        text,
        0.585,
        0.37,
        "Tokens per task",
        "millions, raw, cache reads included  ·  log scale  ·  lower is better",
        groups,
        "tokens",
        lambda v, _: f"{v:g}M",
        log=True,
    )
    y = table(
        text,
        line,
        groups,
        7.45,
        "Table 1  |  Each model at each effort level",
        [
            ("$ per task", lambda g: f"${g['usd']['mean']:.2f}"),
            ("Level total", lambda g: f"${g['usd_total']:.2f}"),
            ("Tokens per task", lambda g: f"{g['tokens']['mean']:.2f}M"),
        ],
    )
    notes(
        text,
        y,
        [
            "List prices per million tokens, input, cached input and output: GPT-6.1 Sol \\$2.00, \\$0.10, \\$10.00; GPT-6 Sol \\$2.00, \\$0.20, \\$10.00;",
            "GPT-5.6 Sol \\$4.00, \\$0.40, \\$20.00. Every cost is recomputed from the run's Codex receipt. Subscription bills differ.",
            *COMMON,
        ],
    )
    write(fig, "sol-family-economics", groups, hashes, ["usd", "usd_total", "tokens", "minutes"])


if __name__ == "__main__":
    main()

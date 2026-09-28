"""Companion card to the v3.16 GPT-6 Luna score card: API-equivalent cost and token use by effort.

Derived from the GPT-5.6 Sol v3.7 economics card. Reads the frozen v3.16
population (private manifest: run ids, receipts, durations), the population
record's per-run API-equivalent costs, and the score card's per-effort table
for the two combined figures. Every cost is recomputed from the run's Codex
receipt at the published list prices and must match the stamped cost.

Six runs hit the flat 3-hour task bound (extra-high 2, max 4). Codex was killed
before it emitted its usage receipt, so those runs have no token count and no
price: they are listed as unpriced, never as $0, and the per-task means cover
the priced runs only. Runtime covers all 23 runs, timeouts at 180 minutes.

    python scripts/cii-v4-board/make_gpt6luna_v316_card.py
    python scripts/cii-v4-board/make_gpt6luna_v316_economics_card.py
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

RUN = ROOT / "runs-code-quality-maintenance-v3.16"
OUTPUT = ROOT / "docs/results/swe-v4-gpt6-luna-2026-09"
LEDGER = OUTPUT / "comparison.json"
SCORES = OUTPUT / "gpt6-luna-v316-efforts.csv"
PAPER, INK, RULE, MUTED = "#f7f5f0", "#171917", "#c6c5bc", "#6b6b66"
MODEL = "gpt6luna"
COLOR = "#10A37F"
NAME = "GPT-6 Luna"
HARNESS = "Codex 0.155.0"
TIMEOUTS = {"extra-high": 2, "max": 4}
PRICES = {"input": 0.10, "cached": 0.01, "output": 0.50}  # GPT-6 Luna list, USD per million
PRICING_VERIFIED = "2026-09-25"


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
    manifest = json.loads((RUN / "private-manifest.json").read_text())
    protocol = json.loads((RUN / "protocol.json").read_text())
    record = json.loads(LEDGER.read_text())
    require(protocol["id"] == "code-quality-maintenance-v3.16", "wrong protocol")
    require(protocol["source_comparison_sha256"] == digest(LEDGER.read_bytes()), "ledger drifted")
    priced = {r["run_id"]: r for r in record["rows"]}
    require({r["run_id"] for r in manifest} == set(priced), "manifest and ledger differ")
    rows = []
    for run in manifest:
        cost = priced[run["run_id"]]
        receipt = run.get("solver_receipt") or {}
        require(type(receipt.get("raw_tokens")) is int, f"no raw token receipt for {run['run_id']}")
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
                "effort": run["effort"],
                "task": run["task"],
                "tokens": receipt["raw_tokens"],
                "usd": cost["api_equivalent_cost_usd"],
                "minutes": run["duration_s"] / 60,
            }
        )
    timeouts = record["excluded"]
    for effort, n in TIMEOUTS.items():
        cell = [e for e in timeouts if e["effort"] == effort]
        require(
            len(cell) == n
            and all(e["finished"] is False and e["duration_s"] >= 10700 for e in cell),
            f"{effort} exclusions are not the {n} timeouts",
        )
    require(len(timeouts) == sum(TIMEOUTS.values()), "unexpected exclusions")
    require(len(rows) + len(timeouts) == 115, "population size")
    return rows, timeouts


def load_scores():
    require(SCORES.exists(), f"{SCORES.name} missing: render the score card first")
    table = {r["effort"]: r for r in csv.DictReader(SCORES.open())}
    require(set(table) == set(LEVELS), "score table levels")
    return {
        e: {
            "judged": float(table[e]["combined_v3"]),
            "timeouts_zero": float(table[e]["combined_timeouts_zero"]),
        }
        for e in LEVELS
    }


def aggregate(rows, timeouts):
    groups = {}
    for effort in LEVELS:
        rs = [r for r in rows if r["effort"] == effort]
        ts = [t for t in timeouts if t["effort"] == effort]
        require(len(rs) + len(ts) == 23, f"{effort} has {len(rs)} priced and {len(ts)} timeouts")
        groups[effort] = {
            "effort": effort,
            "priced": len(rs),
            "unpriced_timeouts": len(ts),
            "usd": mean_se(r["usd"] for r in rs),
            "usd_total": sum(r["usd"] for r in rs),
            "tokens": mean_se(r["tokens"] / 1e6 for r in rs),
            "tokens_total": sum(r["tokens"] for r in rs),
            "minutes": mean_se([r["minutes"] for r in rs] + [t["duration_s"] / 60 for t in ts]),
        }
    totals = {
        "usd": sum(r["usd"] for r in rows),
        "tokens": sum(r["tokens"] for r in rows),
        "hours": (sum(r["minutes"] for r in rows) + sum(t["duration_s"] / 60 for t in timeouts))
        / 60,
        "priced": len(rows),
        "runs": len(rows) + len(timeouts),
    }
    return groups, totals


def main():  # noqa: PLR0915, one linear figure
    rows, timeouts = load()
    groups, totals = aggregate(rows, timeouts)
    scores = load_scores()

    for font in (ROOT / "scripts/rankings-chart").glob("*.ttf"):
        font_manager.fontManager.addfont(font)
    plt.rcParams.update({"font.family": "Geist", "text.color": INK, "svg.fonttype": "path"})
    width_in, height_in = 16, 11.9
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
        "VulcanBench Frontier v4: GPT-6 Luna across effort levels",
        33,
        True,
        heading=True,
    )
    text(
        left,
        2.22,
        "API-equivalent cost and token use at every effort level. 109 of 115 runs priced; "
        "solver inference only, judging excluded.",
        15,
        color=MUTED,
    )

    fig.add_artist(
        plt.Line2D(
            [left + 0.004],
            [yf(2.68)],
            transform=fig.transFigure,
            marker="o",
            color=COLOR,
            markersize=8,
            markeredgecolor=INK,
            markeredgewidth=0.6,
            linestyle="none",
        )
    )
    text(left + 0.018, 2.68, f"{NAME}  ·  {HARNESS}", 15, True)
    text(right, 2.68, "priced n=23, 23, 23, 21, 19 from low to max", 13, ha="right", color=MUTED)

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
            "USD per priced task at list prices  ·  lower is better"
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
        peak = max(groups[e][panel]["mean"] + groups[e][panel]["se"] for e in LEVELS)
        step = next(s for s in (0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10) if peak / s <= 6)
        top = step * math.ceil(peak / step) + step / 2
        ax.set_ylim(0, top)
        ticks = [i * step for i in range(int(top / step) + 1)]
        ax.set_yticks(ticks, [f"{t:g}" for t in ticks])
        ax.set_xlim(-0.6, len(LEVELS) - 0.4)
        xs = list(range(len(LEVELS)))
        ys = [groups[e][panel]["mean"] for e in LEVELS]
        es = [groups[e][panel]["se"] for e in LEVELS]
        ax.bar(
            xs,
            ys,
            width=0.5,
            color=COLOR,
            edgecolor=INK,
            linewidth=0.5,
            yerr=es,
            error_kw={"ecolor": INK, "elinewidth": 0.9, "capsize": 3},
            zorder=2,
        )
        for i, (y, e) in zip(xs, zip(ys, es, strict=True), strict=True):
            label = f"${y:.3f}" if panel == "usd" else f"{y:.2f}M"
            ax.annotate(
                label,
                (i, y + e),
                xytext=(0, 5),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=10,
                fontfamily="IBM Plex Mono",
                color=INK,
            )

    # Table: cost, tokens, time and both combined figures at each effort
    t0 = 7.15
    text(
        left,
        t0,
        "Table 1  |  Cost, tokens, runtime and score at each effort level",
        14.5,
        False,
        heading=True,
    )
    cols = {
        "n": 0.29,
        "usd": 0.40,
        "total": 0.51,
        "tok": 0.62,
        "min": 0.73,
        "judged": 0.835,
        "zero": 0.94,
    }
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Effort", 12.5, True)
    for key, label in (
        ("n", "Priced"),
        ("usd", "$/task"),
        ("total", "Level total"),
        ("tok", "Tokens/task"),
        ("min", "Minutes/task"),
        ("judged", "Combined"),
        ("zero", "Combined"),
    ):
        text(cols[key], t0 + 0.52, label, 12.5, True, ha="right")
    for key, label in (
        ("n", "of 23 runs"),
        ("total", "API-equivalent"),
        ("tok", "raw, millions"),
        ("min", "all 23 runs"),
        ("judged", "judged runs"),
        ("zero", "timeouts as 0"),
    ):
        text(cols[key], t0 + 0.78, label, 10, ha="right", color=MUTED)
    line(left, right, t0 + 0.95, INK, 0.6)
    step, y = 0.35, t0 + 1.25
    for effort in LEVELS:
        g = groups[effort]
        text(left, y, effort.replace("-", " ").capitalize(), 12.5)
        for key, value in (
            ("n", str(g["priced"])),
            ("usd", f"${g['usd']['mean']:.3f}"),
            ("total", f"${g['usd_total']:.2f}"),
            ("tok", f"{g['tokens']['mean']:.2f}M"),
            ("min", f"{g['minutes']['mean']:.1f}"),
            ("judged", f"{scores[effort]['judged']:.2f}"),
            ("zero", f"{scores[effort]['timeouts_zero']:.2f}"),
        ):
            text(cols[key], y, value, 13.5, numeric=True, ha="right")
        y += step
    line(left, right, y - step / 2, RULE, 0.6)
    text(left, y, "Full sweep", 13, True)
    for key, value in (
        ("n", str(totals["priced"])),
        ("usd", f"${totals['usd'] / totals['priced']:.3f}"),
        ("total", f"${totals['usd']:,.2f}"),
        ("tok", f"{totals['tokens'] / 1e6:,.0f}M"),
        ("min", f"{totals['hours']:.1f} h"),
    ):
        text(cols[key], y, value, 14.5, True, numeric=True, ha="right")
    y += step
    line(left, right, y - step / 2, INK, 1.2)
    notes = [
        f"USD at list prices checked {PRICING_VERIFIED}, cache-aware (GPT-6 Luna \\$0.10 input, \\$0.01 cached input, "
        "\\$0.50 output per million), solver inference only; subscription bills differ.",
        "The six runs that hit the 3-hour bound (extra high 2, max 4) ended before Codex reported usage, so they are "
        "unpriced, not \\$0: extra high and max spend is understated.",
        "Runtime covers all 23 runs, timeouts at 180 minutes. Combined figures come from the score card: judged runs "
        "only, and with each timeout counted as 0 over 23 runs.",
        "Whiskers are one task standard error. Tokens are raw solver totals including cache reads.",
    ]
    for i, note in enumerate(notes):
        text(left, y + 0.06 + 0.26 * i, note, 11, color=MUTED)

    out = OUTPUT / "gpt6-luna-v316-economics.png"
    fig.savefig(out, facecolor=PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=PAPER)
    plt.close(fig)
    table = OUTPUT / "gpt6-luna-v316-economics-efforts.csv"
    with table.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "priced_runs",
                "unpriced_timeouts",
                "mean_usd",
                "se_usd",
                "total_usd",
                "mean_tokens",
                "se_tokens",
                "total_tokens",
                "mean_minutes_all_runs",
                "combined_judged",
                "combined_timeouts_zero",
            ]
        )
        for effort in LEVELS:
            g = groups[effort]
            writer.writerow(
                [
                    MODEL,
                    effort,
                    g["priced"],
                    g["unpriced_timeouts"],
                    f"{g['usd']['mean']:.6f}",
                    f"{g['usd']['se']:.6f}",
                    f"{g['usd_total']:.6f}",
                    f"{g['tokens']['mean'] * 1e6:.0f}",
                    f"{g['tokens']['se'] * 1e6:.0f}",
                    g["tokens_total"],
                    f"{g['minutes']['mean']:.4f}",
                    f"{scores[effort]['judged']:.4f}",
                    f"{scores[effort]['timeouts_zero']:.4f}",
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "population": f"runs-code-quality-maintenance-v3.16 population, {totals['runs']} runs, "
            f"{totals['priced']} priced",
            "ledger": LEDGER.name,
            "ledger_sha256": digest(LEDGER.read_bytes()),
            "scores_table": SCORES.name,
            "scores_sha256": digest(SCORES.read_bytes()),
            "pricing_verified": PRICING_VERIFIED,
            "prices_per_million": PRICES,
            "sources": {"openai": "https://developers.openai.com/api/docs/pricing"},
            "totals": totals,
            "groups": groups,
            "unpriced_timeouts": [
                {k: t[k] for k in ("effort", "task", "duration_s")} for t in timeouts
            ],
            "limitations": [
                "Codex receipts report cumulative input, cached input and output per run; cached input is "
                "priced at the cached rate.",
                "Standard tier, short-context rates; Codex keeps each request within its 272K context window, "
                "so no long-context premium is inferred.",
                "Timed-out runs have no usage receipt and are unpriced; level means cover priced runs only.",
                "Estimates cover exposed receipts, not an independently observed API invoice.",
            ],
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

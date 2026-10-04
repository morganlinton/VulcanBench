"""Grok 4.7 beside GPT-6.1 Sol, Claude Opus 5.5 and GPT-6 Astra on VulcanBench Frontier v4.

One card, house layout (make_sol_family_cards helpers): combined score and
minutes per task as charts, then a table of tasks passed, combined score,
Code quality, the Muse Spark 1.3 review score and tokens per task.

Each model is read the way its own published card reads it:
- Grok 4.7 (Cursor): Code quality v3.20, judged by Muse Spark 1.3 and GPT-6.1
  Sol; figures from grok47-cursor-v320-efforts.csv and grok47-cursor-usage-
  efforts.csv. Low to extra-high only (Cursor offers no max).
- GPT-6.1 Sol, Claude Opus 5.5, GPT-6 Astra: the published leaders cards
  (gpt61sol-opus55-astra and its quality companion), judged by Muse and Grok
  4.6 under v3.18, v3.15 and v3.4.

Because the judge pairs differ, the table adds the one judge all four share:
Muse Spark 1.3's reviewed score (the L1 layer), read from each frozen summary.

    python scripts/cii-v4-board/make_grok47_frontier_comparison_card.py
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import ticker

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_sol_family_cards as sf  # noqa: E402

from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

RESULTS = ROOT / "docs/results"
OUTPUT = RESULTS / "swe-v4-grok47-cursor-2026-10"
CARD = "grok47-vs-frontier-leaders"
MODELS = ("grok47cursor", "gpt61sol", "opus55", "astra")
LEADERS = RESULTS / "swe-v4-gpt6-vs-gpt56-2026-09"
SUMMARIES = {
    "grok47cursor": ("runs-code-quality-maintenance-v3.20", "muse"),
    "gpt61sol": ("runs-code-quality-maintenance-v3.18", "muse"),
    "opus55": ("runs-code-quality-maintenance-v3.15", "muse"),
    "astra": ("runs-code-quality-maintenance-v3.4", "muse"),
}

sf.LEVELS = LEVELS
sf.MODELS = MODELS
sf.NAMES = {
    "grok47cursor": "Grok 4.7",
    "gpt61sol": "GPT-6.1 Sol",
    "opus55": "Claude Opus 5.5",
    "astra": "GPT-6 Astra",
}
sf.COLORS = {
    "grok47cursor": "#0A0A0A",
    "gpt61sol": "#0B3D2E",
    "opus55": "#D97757",
    "astra": "#10A37F",
}
sf.MARKERS = {"grok47cursor": "o", "gpt61sol": "s", "opus55": "D", "astra": "^"}
sf.HOLLOW = set()
sf.SOURCES = {
    "grok47cursor": (None, None, None, "Cursor", None),
    "gpt61sol": (None, None, None, "Codex 0.159.0", None),
    "opus55": (None, None, None, "Claude Code", None),
    "astra": (None, None, None, "Codex", None),
}
sf.HEIGHT_IN = 16.5
INK = sf.INK


def rows(path):
    return list(csv.DictReader(path.open()))


def load():
    groups = {m: {} for m in MODELS}
    hashes = {}
    paths = {
        "grok": OUTPUT / "grok47-cursor-v320-efforts.csv",
        "grok_usage": OUTPUT / "grok47-cursor-usage-efforts.csv",
        "leaders": LEADERS / "gpt61sol-opus55-astra-efforts.csv",
        "leaders_quality": LEADERS / "gpt61sol-opus55-astra-quality-efforts.csv",
    }
    for name, path in paths.items():
        hashes[name] = digest(path.read_bytes())
    usage = {r["effort"]: r for r in rows(paths["grok_usage"])}
    for r in rows(paths["grok"]):
        e = r["effort"]
        groups["grok47cursor"][e] = {
            "combined": {"mean": float(r["combined_v3"]), "se": float(r["combined_v3_se"])},
            "judged": int(r["n"]),
            "passed": int(r["passed"]),
            "minutes": {"mean": float(r["minutes"]), "se": 0.0},
            "code_quality": float(r["code_quality"]),
            "tokens": float(usage[e]["mean_tokens_millions"]),
        }
    quality = {(r["model"], r["effort"]): r for r in rows(paths["leaders_quality"])}
    for r in rows(paths["leaders"]):
        m, e = r["model"], r["effort"]
        q = quality[m, e]
        groups[m][e] = {
            "combined": {"mean": float(r["combined"]), "se": float(r["combined_se"])},
            "judged": int(r["judged"]),
            "passed": int(r["passed"]),
            "minutes": {"mean": float(r["mean_minutes"]), "se": 0.0},
            "code_quality": float(q["code_quality"]),
            "tokens": float(q["mean_tokens_millions"]),
        }
    for m, (run, panel) in SUMMARIES.items():
        summary_path = ROOT / run / "summary.json"
        summary = json.loads(summary_path.read_text())
        hashes[f"{m}_summary"] = digest(summary_path.read_bytes())
        for e in groups[m]:
            groups[m][e]["muse"] = summary["groups"][f"{m}/{e}"]["by_panel"][panel]["mean"]
    sf.require(set(groups["grok47cursor"]) == set(LEVELS) - {"max"}, "grok levels")
    for m in MODELS[1:]:
        sf.require(set(groups[m]) == set(LEVELS), f"{m} levels")
    return groups, hashes


def chart(fig, yf, text, x0, w, title, sub, groups, key, fmt):
    tx = 0.06 if x0 < 0.5 else x0 - 0.06
    text(tx, 3.1, title, 21, True, heading=True)
    text(tx, 3.42, sub, 12, color=sf.MUTED)
    ax = fig.add_axes([x0, yf(3.75 + 3.2), w, 3.2 / sf.HEIGHT_IN], facecolor=sf.PAPER)
    xs = list(range(len(LEVELS)))
    ax.set_xticks(xs, [e.replace("-", " ").capitalize() for e in LEVELS])
    ax.tick_params(axis="x", length=0, labelsize=12, pad=8)
    ax.tick_params(axis="y", length=0, labelsize=11, pad=6)
    ax.spines[["top", "right"]].set_visible(False)
    ax.spines[["left", "bottom"]].set_color(sf.RULE)
    ax.grid(axis="y", color=sf.RULE, linewidth=0.5)
    ax.set_axisbelow(True)
    ax.set_xlim(-0.45, len(LEVELS) - 0.55)
    offsets = {m: (i - 1.5) * 0.06 for i, m in enumerate(MODELS)}
    for model in MODELS:
        levels = [e for e in LEVELS if e in groups[model]]
        px = [LEVELS.index(e) + offsets[model] for e in levels]
        ys = [groups[model][e][key]["mean"] for e in levels]
        es = [groups[model][e][key]["se"] for e in levels]
        ax.plot(px, ys, color=sf.COLORS[model], linewidth=2, zorder=2)
        if any(es):
            ax.errorbar(
                px, ys, yerr=es, fmt="none", ecolor=INK, elinewidth=0.8, capsize=2.5, zorder=3
            )
        ax.plot(
            px, ys, linestyle="none", zorder=4, color=sf.COLORS[model], **sf.marker_style(model)
        )
    ax.yaxis.set_major_formatter(ticker.FuncFormatter(fmt))
    return ax


def table(text, line, groups, t0):
    left, right = 0.06, 0.94
    cols = dict(zip(LEVELS, (0.54, 0.64, 0.74, 0.84, 0.94), strict=True))
    text(left, t0, "Table 1  |  Each model at each effort level", 14.5, False, heading=True)
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Model and measure", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], t0 + 0.52, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, t0 + 0.7, INK, 0.6)
    measures = [
        ("Tasks passed (of 23)", lambda c: str(c["passed"])),
        (
            "Combined score",
            lambda c: (
                f"{c['combined']['mean']:.2f}"
                + ("" if c["judged"] == 23 else f" (n={c['judged']})")
            ),
        ),
        ("Code quality (own judges)", lambda c: f"{c['code_quality']:.2f}"),
        ("Muse Spark 1.3 review score", lambda c: f"{c['muse']:.1f}"),
        ("Tokens per task (M)", lambda c: f"{c['tokens']:.2f}"),
    ]
    y, step = t0 + 0.98, 0.31
    for model in MODELS:
        text(left, y, sf.NAMES[model], 13, True)
        for i, (label, fmt) in enumerate(measures):
            text(left + (0.15 if i == 0 else 0.02), y, label, 12, color=sf.MUTED if i == 0 else INK)
            for effort in LEVELS:
                cell = groups[model].get(effort)
                text(
                    cols[effort],
                    y,
                    fmt(cell) if cell else "n/a",
                    12.5,
                    numeric=True,
                    ha="right",
                    color=INK if cell else sf.MUTED,
                )
            y += step
        line(left, right, y - step / 2, sf.RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)
    return y - step / 2


def main():
    groups, hashes = load()
    fig, yf, text, line = sf.figure()
    sf.masthead(
        fig,
        yf,
        text,
        line,
        "Frontier v4: Grok 4.7 beside the leaders",
        "Combined score, time and Code quality at every effort level, 23 tasks per effort.",
        month="October 2026",
    )
    chart(
        fig,
        yf,
        text,
        0.085,
        0.405,
        "Combined score",
        "judged runs, mean ±1 SE  ·  higher is better",
        groups,
        "combined",
        lambda v, _: f"{v:g}",
    )
    chart(
        fig,
        yf,
        text,
        0.585,
        0.37,
        "Mean runtime",
        "minutes per task, all 23 runs  ·  lower is better",
        groups,
        "minutes",
        lambda v, _: f"{v:g}",
    )
    y = table(text, line, groups, 7.65)
    sf.notes(
        text,
        y,
        [
            "Judges differ: Grok 4.7 was judged by Muse Spark 1.3 and GPT-6.1 Sol (Grok 4.6 is not neutral for xAI); the others by Muse and",
            "Grok 4.6. GPT-6.1 Sol rates about 6 points above Muse on the same code, so compare Code quality on the shared Muse row.",
            "Grok 4.7 ran in Cursor, which offers no max level. Its medium cell is judged on 22 runs (one 3-hour timeout, counted as failed).",
            "Claude Opus 5.5 ran with Claude Code's refusal fallback on; its high cell is judged on 22. Each model was judged in its own protocol",
            "run (Grok 4.7 v3.20, GPT-6.1 Sol v3.18, Opus 5.5 v3.15, Astra v3.4) with the same rubric. Tokens are raw, cache reads included.",
        ],
    )
    out = OUTPUT / f"{CARD}.png"
    fig.savefig(out, facecolor=sf.PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=sf.PAPER)
    plt.close(fig)
    table_path = OUTPUT / f"{CARD}-efforts.csv"
    with table_path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            [
                "model",
                "effort",
                "passed",
                "judged",
                "combined",
                "combined_se",
                "code_quality",
                "muse_review",
                "mean_minutes",
                "mean_tokens_millions",
            ]
        )
        for model in MODELS:
            for effort in LEVELS:
                c = groups[model].get(effort)
                if c:
                    writer.writerow(
                        [
                            model,
                            effort,
                            c["passed"],
                            c["judged"],
                            f"{c['combined']['mean']:.4f}",
                            f"{c['combined']['se']:.4f}",
                            f"{c['code_quality']:.4f}",
                            f"{c['muse']:.4f}",
                            f"{c['minutes']['mean']:.4f}",
                            f"{c['tokens']:.4f}",
                        ]
                    )
    save(
        out.with_suffix(".json"),
        {
            "card": CARD,
            "sources_sha256": hashes,
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table_path.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

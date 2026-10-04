"""Grok 4.7 in Cursor on VulcanBench Frontier v4: tasks passed, time and tokens.

The companion to make_grok47cursor_v320_card.py, in place of the usual
economics card: VulcanBench carries no list price for Grok 4.7 and the sweep
ran on the Cursor subscription, so there is no cost panel. Every figure here
covers all 23 runs per level; the medium lodgecore run stopped at the 3-hour
bound counts as failed at its recorded time. Tokens are the raw sums of the
Cursor stream's usage block (input, output, cache reads, cache writes); the
timed-out run has no usage receipt, so medium's tokens average 22 runs.

    python scripts/cii-v4-board/make_grok47cursor_usage_card.py
"""

from __future__ import annotations

import csv
import json
import statistics
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_sol_family_cards as sf  # noqa: E402

from harness.retrospective_judging import digest, save  # noqa: E402

OUTPUT = ROOT / "docs/results/swe-v4-grok47-cursor-2026-10"
LEDGER = OUTPUT / "comparison.json"
CARD = "grok47-cursor-usage"
LEVELS = ("low", "medium", "high", "extra-high")
MODEL = "grok47cursor"

sf.LEVELS = LEVELS
sf.MODELS = (MODEL,)
sf.NAMES = {MODEL: "Grok 4.7"}
sf.COLORS = {MODEL: "#0A0A0A"}
sf.MARKERS = {MODEL: "o"}
sf.HOLLOW = set()
sf.HEIGHT_IN = 12.2
INK = sf.INK


def load():
    record = json.loads(LEDGER.read_text())
    sf.require(not record.get("missing"), "missing runs")
    versions = {r["solver_cli_version"] for r in record["rows"]}
    sf.require(len(versions) == 1, f"several CLI versions {versions}")
    runs = []
    for r in record["rows"]:
        usage = r["solver_receipt"]["usage"]
        runs.append(
            {
                "effort": r["effort"],
                "passed": r["functional"] == 1,
                "minutes": r["duration_s"] / 60,
                "tokens": r["solver_receipt"]["raw_tokens"] / 1e6,
                "output": usage["outputTokens"] / 1e3,
                "cache_share": usage["cacheReadTokens"] / r["solver_receipt"]["raw_tokens"],
            }
        )
    for e in record.get("excluded", []):
        sf.require("Incomplete source run" in e["reason"], f"exclusion {e['reason']}")
        runs.append(
            {
                "effort": e["effort"],
                "passed": False,
                "minutes": e["duration_s"] / 60,
                "tokens": None,
                "output": None,
                "cache_share": None,
            }
        )
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        sf.require(len(rs) == 23, f"{effort}: {len(rs)} runs")
        receipts = [r for r in rs if r["tokens"] is not None]
        groups[effort] = {
            "passed": sum(r["passed"] for r in rs),
            "passed_line": {"mean": sum(r["passed"] for r in rs), "se": 0.0, "n": 23},
            "minutes": sf.mean_se(r["minutes"] for r in rs),
            "median_minutes": statistics.median(r["minutes"] for r in rs),
            "tokens": sf.mean_se(r["tokens"] for r in receipts),
            "output": statistics.mean(r["output"] for r in receipts),
            "cache_share": statistics.mean(r["cache_share"] for r in receipts),
            "receipts": len(receipts),
        }
    return {MODEL: groups}, versions.pop(), digest(LEDGER.read_bytes())


def table(text, line, groups, t0):
    left, right = 0.06, 0.94
    cols = dict(zip(LEVELS, (0.61, 0.72, 0.83, 0.94), strict=True))
    text(left, t0, "Table 1  |  Runs at each effort level", 14.5, False, heading=True)
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Measure", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], t0 + 0.52, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, t0 + 0.7, INK, 0.6)
    measures = [
        ("Tasks passed (of 23)", lambda c: str(c["passed"])),
        ("Mean minutes per task", lambda c: f"{c['minutes']['mean']:.1f}"),
        ("Median minutes per task", lambda c: f"{c['median_minutes']:.1f}"),
        ("Tokens per task (millions)", lambda c: f"{c['tokens']['mean']:.2f}"),
        ("Output tokens per task (thousands)", lambda c: f"{c['output']:.1f}"),
        ("Share of tokens read from cache", lambda c: f"{100 * c['cache_share']:.0f}%"),
    ]
    y, step = t0 + 0.98, 0.33
    for label, fmt in measures:
        text(left, y, label, 12.5)
        for effort in LEVELS:
            text(cols[effort], y, fmt(groups[MODEL][effort]), 13.5, numeric=True, ha="right")
        y += step
    line(left, right, y - step / 2, INK, 1.2)
    return y - step / 2


def main():
    groups, version, record_hash = load()
    sf.SOURCES = {MODEL: (None, None, None, f"Cursor {version}", None)}
    fig, yf, text, line = sf.figure()
    sf.masthead(
        fig,
        yf,
        text,
        line,
        "VulcanBench Frontier v4: Grok 4.7 time and tokens",
        "Tasks passed, minutes and tokens per task at every effort level, all 23 runs per effort.",
        month="October 2026",
    )
    sf.chart(
        fig,
        yf,
        text,
        0.085,
        0.405,
        "Tasks passed",
        "of 23  ·  higher is better",
        groups,
        "passed_line",
        lambda v, _: f"{v:g}",
        ylim=(14, 23.6),
    )
    sf.chart(
        fig,
        yf,
        text,
        0.585,
        0.37,
        "Tokens per task",
        "millions, raw, cache reads included, mean ±1 SE  ·  lower is better",
        groups,
        "tokens",
        lambda v, _: f"{v:g}M",
    )
    y = table(text, line, groups, 7.45)
    sf.notes(
        text,
        y,
        [
            "Medium lodgecore reached the flat 3-hour task bound: it counts as failed and in medium's minutes; it has no usage",
            "receipt, so medium's token figures average 22 runs. Tokens are Cursor's stream usage (input, output, cache reads and",
            "writes). No cost panel: the sweep ran on the Cursor subscription and VulcanBench has no list price for Grok 4.7.",
            "Cursor offers no max level for Grok 4.7.",
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
                "effort",
                "passed",
                "mean_minutes",
                "median_minutes",
                "mean_tokens_millions",
                "mean_output_thousands",
                "cache_read_share",
                "receipts",
            ]
        )
        for effort in LEVELS:
            c = groups[MODEL][effort]
            writer.writerow(
                [
                    effort,
                    c["passed"],
                    f"{c['minutes']['mean']:.4f}",
                    f"{c['median_minutes']:.4f}",
                    f"{c['tokens']['mean']:.4f}",
                    f"{c['output']:.4f}",
                    f"{c['cache_share']:.4f}",
                    c["receipts"],
                ]
            )
    save(
        out.with_suffix(".json"),
        {
            "card": CARD,
            "record": str(LEDGER.relative_to(ROOT)),
            "record_sha256": record_hash,
            "harness_version": version,
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table_path.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

"""Grok 4.7 in Cursor vs. Grok 4.7 in Grok Build on VulcanBench Frontier v4: one card.

The same model on the same 23 tasks, once per effort level in each harness, so
the card isolates the harness. Neither harness exposes a max level for Grok
4.7, so the card runs low to extra-high. Layout follows the house comparison
cards (make_sol_family_cards helpers): combined score and minutes per task as
charts, then a table of tasks passed, Code quality and tokens per task.

Sources, each a frozen Code quality run judged by Muse Spark 1.3 and GPT-6.1
Sol (Grok 4.6 is not neutral for an xAI submission):
- Cursor: v3.20, population docs/results/swe-v4-grok47-cursor-2026-10.
- Grok Build: v3.21, population docs/results/swe-v4-grok47-grokbuild-2026-10.

Combined score averages judged runs; tasks passed and minutes cover all 23
runs per cell, a run stopped at the 3-hour bound counting as failed at its
recorded time. A level with two or more such runs also gets the timeouts-as-0
figure (the two-figure rule). Tokens are the solver receipts' raw totals over
finished runs. There is no cost panel: both legs ran on subscriptions and the
harness carries no list price for Grok 4.7.

    python scripts/cii-v4-board/make_grok47_harness_card.py
    python scripts/cii-v4-board/make_grok47_harness_card.py --preview  # draws what exists, to tmp/
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
import make_gpt6_vs_gpt56_cards as g  # noqa: E402
import make_sol_family_cards as sf  # noqa: E402

from harness.retrospective_judging import digest, save  # noqa: E402

RESULTS = ROOT / "docs/results"
OUTPUT = RESULTS / "swe-v4-grok47-harnesses-2026-10"
CARD = "grok47-cursor-vs-grokbuild"
LEVELS = ("low", "medium", "high", "extra-high")
MODELS = ("grokbuild", "cursor")
PANELS = ("muse", "sol")
SOURCES = {
    # run dir, protocol id, manifest model key, population record, harness label
    "cursor": (
        "runs-code-quality-maintenance-v3.20",
        "code-quality-maintenance-v3.20",
        "grok47cursor",
        "swe-v4-grok47-cursor-2026-10/comparison.json",
        "Cursor",
    ),
    "grokbuild": (
        "runs-code-quality-maintenance-v3.21",
        "code-quality-maintenance-v3.21",
        "grok47grokbuild",
        "swe-v4-grok47-grokbuild-2026-10/comparison.json",
        "Grok Build",
    ),
}

g.PANELS = PANELS
sf.LEVELS = LEVELS
sf.MODELS = MODELS
sf.NAMES = {"grokbuild": "Grok 4.7 in Grok Build", "cursor": "Grok 4.7 in Cursor"}
sf.COLORS = {"grokbuild": "#0A0A0A", "cursor": "#6B6B66"}
sf.MARKERS = {"grokbuild": "o", "cursor": "D"}
sf.HOLLOW = {"cursor"}
sf.HEIGHT_IN = 13.6
INK = sf.INK


def judged(model):
    """run_id -> (combined, Code quality) from the frozen summary, or {} in preview without one."""
    run_name, protocol_id, key, _, _ = SOURCES[model]
    run_dir = ROOT / run_name
    if not (run_dir / "summary.json").exists():
        return None, None
    summary = json.loads((run_dir / "summary.json").read_text())
    g.require(summary["protocol"] == protocol_id, f"{run_name}: wrong protocol")
    g.require(set(summary["passing_panels"]) <= set(PANELS), f"{run_name}: panels")
    manifest = {r["id"]: r for r in json.loads((run_dir / "private-manifest.json").read_text())}
    out = {}
    for entry in summary["rows"]:
        if entry["model"] != key or not entry.get("published"):
            continue
        run = manifest[entry["id"]]
        cq = g.code_quality(entry)
        out[run["run_id"]] = (g.composite(run, cq), cq)
    hashes = {
        "summary": digest((run_dir / "summary.json").read_bytes()),
        "protocol": digest((run_dir / "protocol.json").read_bytes()),
        "passing_panels": sorted(summary["passing_panels"]),
    }
    return out, hashes


def load(model, preview):
    _, _, key, record_name, _ = SOURCES[model]
    path = RESULTS / record_name
    if preview and not path.exists():
        return None, None, None
    record = json.loads(path.read_text())
    g.require(not record.get("missing"), f"{model}: missing runs")
    scores, hashes = judged(model)
    g.require(preview or scores is not None, f"{model}: not judged yet")
    hashes = hashes or {}
    hashes["record"] = digest(path.read_bytes())
    versions = {r["solver_cli_version"] for r in record["rows"]}
    g.require(len(versions) == 1, f"{model}: several CLI versions {versions}")
    runs = []
    for r in record["rows"]:
        g.require(r["model"] == key, f"{model}: unexpected row {r['model']}")
        score = (scores or {}).get(r["run_id"])
        runs.append(
            {
                "effort": r["effort"],
                "passed": r["functional"] == 1,
                "minutes": r["duration_s"] / 60,
                "tokens": r["solver_receipt"]["raw_tokens"] / 1e6,
                "combined": score[0] if score else None,
                "cq": score[1] if score else None,
                "timeout": False,
            }
        )
    for r in record.get("excluded", []):
        g.require("Incomplete source run" in r["reason"], f"{model}: exclusion {r['reason']}")
        runs.append(
            {
                "effort": r["effort"],
                "passed": False,
                "minutes": r["duration_s"] / 60,
                "tokens": None,
                "combined": None,
                "cq": None,
                "timeout": True,
            }
        )
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        g.require(len(rs) == 23, f"{model} {effort}: {len(rs)} runs")
        scored = [r["combined"] for r in rs if r["combined"] is not None]
        timeouts = sum(r["timeout"] for r in rs)
        groups[effort] = {
            "passed": sum(r["passed"] for r in rs),
            "timeouts": timeouts,
            "judged": len(scored),
            "combined": g.mean_se(scored),
            "combined_timeouts_zero": (
                statistics.mean(scored + [0.0] * timeouts) if scored and timeouts > 1 else None
            ),
            "code_quality": statistics.mean(r["cq"] for r in rs if r["cq"] is not None)
            if scored
            else None,
            "minutes": g.mean_se(r["minutes"] for r in rs),
            "tokens": g.mean_se(r["tokens"] for r in rs if r["tokens"] is not None),
        }
    return groups, hashes, versions.pop()


def table(text, line, groups, models, t0):
    left, right = 0.06, 0.94
    cols = dict(zip(LEVELS, (0.58, 0.70, 0.82, 0.94), strict=True))
    text(left, t0, "Table 1  |  Each harness at each effort level", 14.5, False, heading=True)
    line(left, right, t0 + 0.28, INK, 1.2)
    text(left, t0 + 0.52, "Harness and measure", 12.5, True)
    for effort in LEVELS:
        text(cols[effort], t0 + 0.52, effort.replace("-", " ").capitalize(), 12.5, True, ha="right")
    line(left, right, t0 + 0.7, INK, 0.6)
    measures = [
        ("Tasks passed (of 23)", lambda c: str(c["passed"])),
        (
            "Combined score",
            lambda c: (
                "pending"
                if c["combined"]["mean"] is None
                else f"{c['combined']['mean']:.2f}"
                + ("" if c["judged"] == 23 else f" (n={c['judged']})")
            ),
        ),
        (
            "Code quality",
            lambda c: "pending" if c["code_quality"] is None else f"{c['code_quality']:.2f}",
        ),
        ("Tokens per task (M)", lambda c: f"{c['tokens']['mean']:.2f}"),
    ]
    if any(groups[m][e]["combined_timeouts_zero"] is not None for m in models for e in LEVELS):
        measures.insert(
            2,
            (
                "Combined, timeouts as 0",
                lambda c: (
                    ""
                    if c["combined_timeouts_zero"] is None
                    else f"{c['combined_timeouts_zero']:.2f}"
                ),
            ),
        )
    y, step = t0 + 0.98, 0.33
    for model in models:
        text(left, y, sf.NAMES[model], 13, True)
        for i, (label, fmt) in enumerate(measures):
            text(left + (0.16 if i == 0 else 0.02), y, label, 12, color=sf.MUTED if i == 0 else INK)
            for effort in LEVELS:
                text(cols[effort], y, fmt(groups[model][effort]), 13, numeric=True, ha="right")
            y += step
        line(left, right, y - step / 2, sf.RULE, 0.6)
    line(left, right, y - step / 2, INK, 1.2)
    return y - step / 2


def main():
    preview = "--preview" in sys.argv
    groups, hashes, versions = {}, {}, {}
    for model in MODELS:
        loaded, h, version = load(model, preview)
        if loaded is not None:
            groups[model], hashes[model], versions[model] = loaded, h, version
    models = tuple(m for m in MODELS if m in groups)
    g.require(models, "no harness has data yet")
    sf.MODELS = models
    sf.SOURCES = {m: (None, None, None, f"{SOURCES[m][4]} {versions[m]}", None) for m in models}

    fig, yf, text, line = sf.figure()
    sf.masthead(
        fig,
        yf,
        text,
        line,
        "Grok 4.7: Cursor vs. Grok Build",
        "VulcanBench Frontier v4, the same 23 tasks once per effort level in each harness. "
        "Code quality judged by Muse Spark 1.3 and GPT-6.1 Sol.",
        month="October 2026",
    )
    judged_groups = {
        m: {e: {"combined": groups[m][e]["combined"]} for e in LEVELS}
        for m in models
        if all(groups[m][e]["combined"]["mean"] is not None for e in LEVELS)
    }
    if judged_groups:
        sf.MODELS = tuple(judged_groups)
        sf.chart(
            fig,
            yf,
            text,
            0.085,
            0.405,
            "Combined score",
            "judged runs, mean ±1 SE  ·  higher is better",
            judged_groups,
            "combined",
            lambda v, _: f"{v:g}",
        )
        sf.MODELS = models
    sf.chart(
        fig,
        yf,
        text,
        0.585,
        0.37,
        "Mean runtime",
        "minutes per task, all 23 runs, mean ±1 SE  ·  lower is better",
        groups,
        "minutes",
        lambda v, _: f"{v:g}",
    )
    y = table(text, line, groups, models, 7.45)
    timeouts = [
        f"{sf.NAMES[m]} {e.replace('-', ' ')} ({groups[m][e]['timeouts']})"
        for m in models
        for e in LEVELS
        if groups[m][e]["timeouts"]
    ]
    notes = [
        "Combined score averages judged runs: 50% functional, 8.5% quality, 8.5% security and 33% Code quality. Tasks passed and minutes",
        "cover all 23 runs per cell. Runs stopped at the 3-hour bound count as failed at their recorded time and are not judged"
        + (f": {', '.join(timeouts)}." if timeouts else "; none occurred."),
        "Code quality judges: Muse Spark 1.3 and GPT-6.1 Sol (Grok 4.6, the usual second judge, is not neutral for an xAI model).",
        "Tokens are raw solver totals (input, output, cache reads and writes) over finished runs. No cost panel: both harnesses ran on",
        "subscriptions and VulcanBench has no list price for Grok 4.7. Neither harness offers a max level for Grok 4.7.",
    ]
    if preview:
        notes.append("PREVIEW: not for publication; drawn before every harness was judged.")
    sf.notes(text, y, notes)

    out_dir = ROOT / "tmp" if preview else OUTPUT
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / f"{CARD}.png"
    fig.savefig(out, facecolor=sf.PAPER)
    if not preview:
        fig.savefig(out.with_suffix(".svg"), facecolor=sf.PAPER)
    plt.close(fig)
    if preview:
        print(out)
        return
    table_path = OUTPUT / f"{CARD}-efforts.csv"
    fields = ["passed", "timeouts", "judged", "combined", "combined_timeouts_zero"]
    fields += ["code_quality", "minutes", "tokens"]
    with table_path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["harness", "effort", *fields])
        for model in models:
            for effort in LEVELS:
                c = groups[model][effort]
                row = [model, effort]
                for f in fields:
                    v = c[f]
                    v = v["mean"] if isinstance(v, dict) else v
                    row.append(f"{v:.4f}" if isinstance(v, float) else ("" if v is None else v))
                writer.writerow(row)
    save(
        out.with_suffix(".json"),
        {
            "card": CARD,
            "sources": {
                m: {"run": SOURCES[m][0], "harness_version": versions[m], **hashes[m]}
                for m in models
            },
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table_path.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

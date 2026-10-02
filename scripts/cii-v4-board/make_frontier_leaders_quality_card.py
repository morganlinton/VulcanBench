"""GPT-6.1 Sol, Claude Opus 5.5 and GPT-6 Astra on VulcanBench Frontier v4: time, tokens and Code quality.

Companion to make_frontier_leaders_card.py (combined score and cost). Draws
minutes per task and raw tokens per task at every effort level, and a table of
Code quality and its layers (human readability, maintainability, intent
recovery), reusing the Sol-family card layout.

Sources, each as its own report read it:
- GPT-6.1 Sol: Code quality v3.18; tokens are Codex receipts' raw totals.
- Claude Opus 5.5 (with fallback): v3.15; tokens are Claude Code's final
  modelUsage summed over every serving model (input, cache reads, cache
  writes, output), the figure its report publishes. High depotcore was not
  judged (a classifier stop left no code); its time and tokens still count.
- GPT-6 Astra: v3.4; tokens are Codex receipts' raw totals.

Code quality follows the frozen summaries: the mean of the panels with a valid
review, so GPT-6.1 Sol's medium paddockcore is Muse's alone. Each model's
per-level Code quality must match its published efforts table.

    python scripts/cii-v4-board/make_frontier_leaders_quality_card.py
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

from harness.retrospective_judging import LEVELS, digest, save  # noqa: E402

RESULTS = ROOT / "docs/results"
OUTPUT = RESULTS / "swe-v4-gpt6-vs-gpt56-2026-09"
CARD = "gpt61sol-opus55-astra-quality"
MODELS = ("gpt61sol", "opus55", "astra")
PANELS = ("muse", "grok")
SPLIT = {"l1": 0.24, "l2": 0.09}
RUNS = {
    "gpt61sol": (
        "runs-code-quality-maintenance-v3.18",
        "code-quality-maintenance-v3.18",
        "gpt61sol",
    ),
    "opus55": ("runs-code-quality-maintenance-v3.15", "code-quality-maintenance-v3.15", "opus55"),
    "astra": ("runs-code-quality-maintenance-v3.4", "code-quality-maintenance-v3.4", "astra"),
}
PUBLISHED = {
    "gpt61sol": RESULTS / "swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-efforts.csv",
    "opus55": RESULTS / "swe-v4-opus55-2026-09/opus55-v315-efforts.csv",
}

sf.MODELS = MODELS
sf.NAMES = {"gpt61sol": "GPT-6.1 Sol", "opus55": "Claude Opus 5.5", "astra": "GPT-6 Astra"}
sf.COLORS = {"gpt61sol": "#0B3D2E", "opus55": "#D97757", "astra": "#10A37F"}
sf.MARKERS = {"gpt61sol": "o", "opus55": "s", "astra": "D"}
sf.HOLLOW = set()
sf.SOURCES = {
    "gpt61sol": (None, None, None, "Codex 0.159.0", None),
    "opus55": (None, None, None, "Claude Code 2.1.280", None),
    "astra": (None, None, None, "Codex", None),
}
sf.HEIGHT_IN = 14.5


def quality(entry):
    """Code quality and its layers from the panels with a valid review, as the frozen summary publishes it."""
    reviewed = [entry["panels"][p] for p in PANELS if entry["panels"][p]["l1"] is not None]
    sf.require(reviewed, f"{entry['id']}: no valid review")
    l1 = statistics.mean(p["l1"]["score"] for p in reviewed)
    l2s = [p["l2"] for p in reviewed if p["l2"] is not None]
    cq = (SPLIT["l1"] * l1 + SPLIT["l2"] * statistics.mean(l2s)) / 0.33 if l2s else l1
    published = (entry.get("published") or {}).get("code_quality")
    sf.require(
        published is None or abs(published - cq) < 1e-6,
        f"{entry['id']}: {cq} vs summary {published}",
    )
    return {
        "code_quality": cq,
        "readability": statistics.mean(p["l1"]["readability"] for p in reviewed),
        "maintainability": statistics.mean(p["l1"]["maintainability"] for p in reviewed),
        "intent": statistics.mean(l2s) if l2s else None,
    }


def judged(model):
    run_name, protocol_id, key = RUNS[model]
    run_dir = ROOT / run_name
    summary = json.loads((run_dir / "summary.json").read_text())
    sf.require(summary["protocol"] == protocol_id, f"{run_name}: wrong protocol")
    sf.require(set(summary["passing_panels"]) == set(PANELS), f"{run_name}: panels")
    manifest = {r["id"]: r for r in json.loads((run_dir / "private-manifest.json").read_text())}
    out = {}
    for entry in summary["rows"]:
        if entry["model"] == key and entry.get("published"):
            out[manifest[entry["id"]]["run_id"]] = (quality(entry), manifest[entry["id"]])
    return out, digest((run_dir / "summary.json").read_bytes())


def claude_raw_tokens(stream: Path) -> int:
    last = None
    for line in stream.read_text().splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") == "result":
            last = event
    usage = (last or {}).get("modelUsage") or {}
    keys = ("inputTokens", "cacheReadInputTokens", "cacheCreationInputTokens", "outputTokens")
    return sum(v.get(k, 0) for v in usage.values() for k in keys)


def runs_for(model, scores):
    if model == "astra":
        return [
            {
                "effort": run["effort"],
                "minutes": run["duration_s"] / 60,
                "tokens": run["solver_receipt"]["raw_tokens"] / 1e6,
                "q": q,
            }
            for q, run in scores.values()
        ]
    ledger_name = {
        "gpt61sol": "swe-v4-gpt61-sol-2026-09/comparison.json",
        "opus55": "swe-v4-opus55-2026-09/comparison.json",
    }[model]
    ledger = json.loads((RESULTS / ledger_name).read_text())
    runs = []
    for r in ledger["rows"] + ledger.get("excluded", []):
        directory = (
            Path(r["source_directory"])
            if "source_directory" in r
            else ROOT / "runs-effort-opus55" / r["effort"] / r["run_id"]
        )
        if model == "opus55":
            tokens = claude_raw_tokens(directory / "cli-agent-stream.jsonl")
        else:
            tokens = r["solver_receipt"]["raw_tokens"]
        runs.append(
            {
                "effort": r["effort"],
                "minutes": r["duration_s"] / 60,
                "tokens": tokens / 1e6,
                "q": scores[r["run_id"]][0] if r["run_id"] in scores else None,
            }
        )
    return runs


def aggregate(model):
    scores, summary_hash = judged(model)
    runs = runs_for(model, scores)
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        sf.require(len(rs) == 23, f"{model} {effort}: {len(rs)} runs")
        qs = [r["q"] for r in rs if r["q"] is not None]
        groups[effort] = {
            "minutes": sf.mean_se(r["minutes"] for r in rs),
            "tokens": sf.mean_se(r["tokens"] for r in rs),
            "judged": len(qs),
            "code_quality": statistics.mean(q["code_quality"] for q in qs),
            "readability": statistics.mean(q["readability"] for q in qs),
            "maintainability": statistics.mean(q["maintainability"] for q in qs),
            "intent": statistics.mean(q["intent"] for q in qs if q["intent"] is not None),
        }
    if model in PUBLISHED:
        table = {r["effort"]: r for r in csv.DictReader(PUBLISHED[model].open())}
        for effort in LEVELS:
            want = float(table[effort]["code_quality"])
            sf.require(
                abs(want - groups[effort]["code_quality"]) < 1e-3,
                f"{model} {effort}: Code quality drift",
            )
    return groups, summary_hash


def main():
    groups, hashes = {}, {}
    for model in MODELS:
        groups[model], hashes[model] = aggregate(model)

    fig, yf, text, line = sf.figure()
    sf.masthead(
        fig,
        yf,
        text,
        line,
        "VulcanBench Frontier v4: GPT-6.1 Sol, Opus 5.5 and Astra",
        "Time and tokens per task at every effort level, 23 tasks per effort, and Code quality judged by Muse Spark 1.3 and Grok 4.6.",
        month="October 2026",
    )
    sf.chart(
        fig,
        yf,
        text,
        0.085,
        0.405,
        "Mean runtime",
        "minutes per task, all 23 runs  ·  lower is better",
        groups,
        "minutes",
        lambda v, _: f"{v:g}",
    )
    sf.chart(
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
    y = sf.table(
        text,
        line,
        groups,
        7.45,
        "Table 1  |  Code quality at each effort level (/100, mean of both judges)",
        [
            ("Code quality", lambda c: f"{c['code_quality']:.2f}"),
            ("Human readability", lambda c: f"{c['readability']:.1f}"),
            ("Maintainability", lambda c: f"{c['maintainability']:.1f}"),
            ("Intent recovery", lambda c: f"{c['intent']:.1f}"),
        ],
    )
    sf.notes(
        text,
        y,
        [
            "Code quality is 33% of the combined score: 24 points from the two judges' reviews (readability and maintainability) and 9 from",
            "intent recovery. Each model was judged in its own protocol run (GPT-6.1 Sol v3.18, Opus 5.5 v3.15, Astra v3.4) with the same rubric.",
            "Opus 5.5 high is judged on 22 runs; GPT-6.1 Sol medium paddockcore is scored from Muse alone. Time and tokens cover all 23 runs.",
            "Tokens: Codex receipts' raw totals for the GPT models; for Opus 5.5, Claude Code's usage summed over every serving model (its",
            "refusal fallback to Opus 4.8 included), with cache reads and writes. GPT-6.1 Sol's sweep partly overlapped other judging on the machine.",
        ],
    )
    OUTPUT.mkdir(parents=True, exist_ok=True)
    out = OUTPUT / f"{CARD}.png"
    fig.savefig(out, facecolor=sf.PAPER)
    fig.savefig(out.with_suffix(".svg"), facecolor=sf.PAPER)
    plt.close(fig)
    table_path = OUTPUT / f"{CARD}-efforts.csv"
    fields = ["code_quality", "readability", "maintainability", "intent", "judged"]
    with table_path.open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["model", "effort", "mean_minutes", "mean_tokens_millions", *fields])
        for model in MODELS:
            for effort in LEVELS:
                c = groups[model][effort]
                writer.writerow(
                    [model, effort, f"{c['minutes']['mean']:.4f}", f"{c['tokens']['mean']:.4f}"]
                    + [f"{c[f]:.4f}" if isinstance(c[f], float) else c[f] for f in fields]
                )
    save(
        out.with_suffix(".json"),
        {
            "card": CARD,
            "summaries": {m: {"run": RUNS[m][0], "summary_sha256": hashes[m]} for m in MODELS},
            "png_sha256": digest(out.read_bytes()),
            "supporting_table": table_path.name,
        },
    )
    print(out)


if __name__ == "__main__":
    main()

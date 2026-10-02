"""GPT-6.1 Sol beside Claude Opus 5.5 and GPT-6 Astra on VulcanBench Frontier v4: one card.

Same layout as the GPT-6 vs. GPT-5.6 comparison cards (it reuses their
drawing code), with each model read the way its own report read it:

- GPT-6.1 Sol: Code quality v3.18, Codex receipts re-priced at $2.00 / $0.10 /
  $10.00 per million (make_gpt6_vs_gpt56_cards.load).
- Claude Opus 5.5 (with fallback): v3.15. Cost is Claude Code's own list-price
  total per run (every serving model, the Opus 4.8 fallback turns included),
  as on its card. High depotcore was excluded from judging (a classifier stop
  left no code), so high is judged on 22; that run counts as a failed task at
  its recorded time and cost.
- GPT-6 Astra: v3.4 (the launch-week sweep that published it). Cost is that
  report's central API-equivalent estimate.

Combined scores are judged-run means and must match each model's published
efforts table; passes, minutes and cost cover all 23 runs per level.

    python scripts/cii-v4-board/make_frontier_leaders_card.py
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_gpt6_vs_gpt56_cards as g  # noqa: E402

from harness.retrospective_judging import LEVELS, digest  # noqa: E402

RESULTS = ROOT / "docs/results"
CARD = "gpt61sol-opus55-astra"
MODELS = ("gpt61sol", "opus55", "astra")

g.NAMES.update({"opus55": "Claude Opus 5.5", "astra": "GPT-6 Astra"})
g.HARNESS.update({"opus55": "Claude Code 2.1.280", "astra": "Codex"})
g.COLORS.update({"gpt61sol": "#0B3D2E", "opus55": "#D97757", "astra": "#10A37F"})
g.MARKERS.update({"opus55": "s", "astra": "D"})
g.SOURCES.update(
    {
        "opus55": (
            "runs-code-quality-maintenance-v3.15",
            "code-quality-maintenance-v3.15",
            "opus55",
            "swe-v4-opus55-2026-09/comparison.json",
            (4.00, 0.20, 20.00),
        ),
        "astra": (
            "runs-code-quality-maintenance-v3.4",
            "code-quality-maintenance-v3.4",
            "astra",
            "swe-v4-astra-fable51-2026-09/api-equivalent-costs.json",
            (10.00, 1.00, 50.00),
        ),
    }
)
g.PRICING_VERIFIED.update({"opus55": "2026-09-22", "astra": "2026-09-06"})
PUBLISHED = {
    "gpt61sol": RESULTS / "swe-v4-gpt61-sol-2026-09/gpt61-sol-v318-efforts.csv",
    "opus55": RESULTS / "swe-v4-opus55-2026-09/opus55-v315-efforts.csv",
}


def judged(model):
    """run_id -> combined score from the frozen summary, the way every comparison card computes it."""
    run_name, protocol_id, key, _, _ = g.SOURCES[model]
    run_dir = ROOT / run_name
    summary = json.loads((run_dir / "summary.json").read_text())
    g.require(summary["protocol"] == protocol_id, f"{run_name}: wrong protocol")
    g.require(set(summary["passing_panels"]) == set(g.PANELS), f"{run_name}: panels")
    manifest = {r["id"]: r for r in json.loads((run_dir / "private-manifest.json").read_text())}
    out = {}
    for entry in summary["rows"]:
        if entry["model"] != key or not entry.get("published"):
            continue
        run = manifest[entry["id"]]
        out[run["run_id"]] = (g.composite(run, g.code_quality(entry)), run)
    hashes = {
        "summary": digest((run_dir / "summary.json").read_bytes()),
        "protocol": digest((run_dir / "protocol.json").read_bytes()),
    }
    return out, hashes


def group(model, runs):
    groups = {}
    for effort in LEVELS:
        rs = [r for r in runs if r["effort"] == effort]
        g.require(len(rs) == 23, f"{model} {effort}: {len(rs)} runs")
        scored = [r["combined"] for r in rs if r["combined"] is not None]
        priced = [r["usd"] for r in rs if r["usd"] is not None]
        groups[effort] = {
            "n_runs": 23,
            "n_judged": len(scored),
            "timeouts": 0,
            "combined": g.mean_se(scored),
            "combined_timeouts_zero": None,
            "passed": sum(r["passed"] for r in rs),
            "minutes": g.mean_se(r["minutes"] for r in rs),
            "usd": g.mean_se(priced),
            "n_priced": len(priced),
        }
    return groups


def load_opus55():
    scores, hashes = judged("opus55")
    ledger_path = RESULTS / g.SOURCES["opus55"][3]
    ledger = json.loads(ledger_path.read_text())
    hashes["ledger"] = digest(ledger_path.read_bytes())
    runs = []
    for r in ledger["rows"] + ledger.get("excluded", []):
        run_id = r["run_id"]
        directory = (
            Path(r["source_directory"])
            if "source_directory" in r
            else ROOT / "runs-effort-opus55" / r["effort"] / run_id
        )
        summary = json.loads((directory / "summary.json").read_text())
        g.require(summary["model"] == "claude-code:claude-opus-5-5", f"{run_id}: solver")
        runs.append(
            {
                "effort": r["effort"],
                "passed": summary["scores"]["functional"] == 1,
                "minutes": summary["duration_s"] / 60,
                "usd": summary["economics"]["cli_reported_cost_usd"],
                "combined": scores[run_id][0] if run_id in scores else None,
            }
        )
    g.require(len(scores) == 114, f"opus55: {len(scores)} judged")
    return group("opus55", runs), hashes


def load_astra():
    scores, hashes = judged("astra")
    ledger_path = RESULTS / g.SOURCES["astra"][3]
    ledger = json.loads(ledger_path.read_text())
    hashes["ledger"] = digest(ledger_path.read_bytes())
    costs = {r["run_id"]: r["estimated_usd"] for r in ledger["rows"] if r["model"] == "astra"}
    g.require(set(costs) == set(scores), "astra: ledger and judged runs differ")
    runs = [
        {
            "effort": run["effort"],
            "passed": run["functional"] == 1,
            "minutes": run["duration_s"] / 60,
            "usd": costs[run_id],
            "combined": combined,
        }
        for run_id, (combined, run) in scores.items()
    ]
    return group("astra", runs), hashes


def check_published(model, groups):
    if model not in PUBLISHED:
        return
    table = {r["effort"]: r for r in csv.DictReader(PUBLISHED[model].open())}
    for effort in LEVELS:
        want = float(table[effort]["combined_v3"])
        got = groups[effort]["combined"]["mean"]
        g.require(
            abs(want - got) < 1e-3, f"{model} {effort}: {got} differs from its published {want}"
        )


def main():
    groups, hashes = {}, {}
    runs, hashes["gpt61sol"] = g.load("gpt61sol")
    groups["gpt61sol"] = g.aggregate("gpt61sol", runs)
    groups["opus55"], hashes["opus55"] = load_opus55()
    groups["astra"], hashes["astra"] = load_astra()
    for model in MODELS:
        check_published(model, groups[model])
    notes = [
        "Combined score averages judged runs, as on each model's own card. Tasks passed, cost and minutes cover all 23 runs per cell.",
        "Claude Opus 5.5 ran with Claude Code's refusal fallback on (Artificial Analysis convention); its cost is Claude Code's list-price total,",
        "fallback turns included. Its high cell is judged on 22 runs (a classifier stop left no code on depotcore; that run counts as failed).",
        "GPT-6.1 Sol medium paddockcore is scored from Muse alone. GPT-6 Astra's cost is its report's central estimate (no long-context premium).",
        "Each model was judged in its own protocol run (GPT-6.1 Sol v3.18, Opus 5.5 v3.15, Astra v3.4) with the same rubric and judges.",
        "List prices per million tokens, input and output: GPT-6.1 Sol \\$2.00 and \\$10.00; Claude Opus 5.5 \\$4.00 and \\$20.00;",
        "GPT-6 Astra \\$10.00 and \\$50.00. Solver inference only; subscription bills differ. Harnesses: Codex 0.159.0, Claude Code 2.1.280, Codex.",
    ]
    g.draw(
        CARD,
        MODELS,
        "GPT-6.1 Sol, Opus 5.5 and Astra",
        groups,
        hashes,
        notes_override=notes,
        month="October 2026",
    )


if __name__ == "__main__":
    main()

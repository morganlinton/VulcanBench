"""Build the Code quality judging population for the Claude Fable 5.2 Frontier v4 sweep.

Reads every finished run under runs-effort-fable52/<level>/ and writes the
comparison record the v3.30 runner freezes against, in the same row shape as
the Sonnet 5.5 and GPT-6.1 Terra records. Unfinished runs are listed under
"excluded" with the reason; a task and level with no attempt at all is listed
under "missing". The sweep runs from bench/2026-11-09-fable52-frontier-v4, so
its task hashes match the suite lock and no hash bridge is needed.

v3.30 judges Low to Extra high while Max waits for the weekly Fable limit to
reset (owner request in chat, 2026-11-10). ``--level max`` writes Max alone to
``comparison-max.json`` for the round that follows, the way Muse Wonder Full's
Max was built for v3.26.5.

Fable 5.2 ran with Claude Code's refusal fallback on (docs/DECISIONS.md,
2026-09-24). ``fallback`` is True when any assistant reply came from another
model, and ``fallback_replies`` counts replies by model from the stream.

When this runs from a worktree, link the sweep folder in first:
``ln -s ~/dev/VulcanBench/runs-effort-fable52 runs-effort-fable52``.

    python scripts/cii-v4-board/build_fable52_population.py
    python scripts/cii-v4-board/build_fable52_population.py --level max
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
RESULTS = ROOT / "docs/results/swe-v4-fable52-2026-11"
SWEEP_ROOT = "runs-effort-fable52"
SOLVER = "claude-code:claude-fable-5-2"
SERVED = "claude-fable-5-2"
FIRST_LEVELS = ("low", "medium", "high", "extra-high")
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def _replies(run: Path) -> dict[str, int]:
    """Assistant replies by serving model, straight from the CLI stream."""
    counts: Counter = Counter()
    for line in (run / "cli-agent-stream.jsonl").open(errors="replace"):
        if '"assistant"' not in line:
            continue
        event = json.loads(line)
        if event.get("type") == "assistant":
            counts[event["message"].get("model")] += 1
    return dict(sorted(counts.items()))


def build(levels: tuple[str, ...]) -> dict:
    rows, excluded, missing = [], [], []
    for level in levels:
        seen = set()
        for run in sorted((ROOT / SWEEP_ROOT / level).glob("legacy-*/")):
            summary_path = run / "summary.json"
            if not summary_path.exists():
                continue
            summary = json.loads(summary_path.read_text())
            if summary["model"] != SOLVER:
                raise ValueError(f"{run}: unexpected solver {summary['model']}")
            if summary["effort"]["requested"] != level:
                raise ValueError(
                    f"{run}: effort {summary['effort']['requested']} in the {level} folder"
                )
            seen.add(summary["task_id"])
            try:
                data = base.inputs(run, TASKS)
            except ValueError as exc:
                excluded.append(
                    {
                        "model": "fable52",
                        "effort": level,
                        "task": summary["task_id"],
                        "run_id": run.name,
                        "reason": str(exc),
                        "finished": summary.get("finished"),
                        "duration_s": summary["duration_s"],
                        "functional": summary["scores"]["functional"],
                    }
                )
                continue
            scores = summary["scores"]
            replies = _replies(run)
            rows.append(
                {
                    "model": "fable52",
                    "effort": level,
                    "task": summary["task_id"],
                    "run_id": run.name,
                    "functional": scores["functional"],
                    "quality": scores["quality"],
                    "security": scores["security"],
                    "fallback": any(k != SERVED for k in replies),
                    "fallback_replies": replies,
                    "duration_s": summary["duration_s"],
                    "reported_tokens": summary["total_tokens"],
                    "solver_receipt": solver_receipt(run, summary),
                    "source_directory": str(run),
                    "source_hashes": data["source_hashes"],
                    "started_at": summary["started_at"],
                    "finished_at": summary["finished_at"],
                    "solver_cli_version": summary["cli_agent"]["harness_version"],
                    "api_equivalent_cost_usd": summary["economics"]["api_equivalent_cost_usd"],
                }
            )
        for task in TASK_IDS:
            if task not in seen:
                missing.append(
                    {"model": "fable52", "effort": level, "task": task, "reason": MISSING_REASON}
                )
    cells: dict[str, int] = {}
    for r in rows:
        cells[f"{r['model']}/{r['effort']}"] = cells.get(f"{r['model']}/{r['effort']}", 0) + 1
    dupes = len(rows) - len({(r["model"], r["effort"], r["task"]) for r in rows})
    if dupes:
        raise ValueError(f"{dupes} duplicate model/effort/task rows")
    return {
        "suite": "coding-intelligence-index-v4",
        "models": {"fable52": "Claude Fable 5.2 in Claude Code 2.1.321 (refusal fallback on)"},
        "levels": {"fable52": list(levels)},
        "cells": cells,
        "rows": rows,
        "excluded": excluded,
        "missing": missing,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--level", choices=("max",), help="build Max alone into comparison-max.json"
    )
    args = parser.parse_args()
    levels = ("max",) if args.level == "max" else FIRST_LEVELS
    output = RESULTS / ("comparison-max.json" if args.level == "max" else "comparison.json")
    record = build(levels)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    rows = record["rows"]
    print(
        f"{len(rows)} rows, {len(record['excluded'])} excluded, {len(record['missing'])} missing; "
        f"cells: {record['cells']}; fallback rows: {sum(r['fallback'] for r in rows)}"
    )
    for e in record["excluded"]:
        print("excluded:", e["effort"], e["task"], e["reason"])
    for e in record["missing"]:
        print("missing:", e["effort"], e["task"])


if __name__ == "__main__":
    main()

"""Build the judging population for the Gemini 4 Neon effort sweep.

Reads every finished run under runs-effort-gemini4-neon-agy/<level>/ (Low to
Extra high; Neon has no Max) and writes the comparison record the v3.32 runner
freezes against, in the same row shape as the GPT-6.2 Sol record. Unfinished
runs (the 3-hour timeouts) are listed under "excluded" with the reason; a task and level with no attempt at all is listed
under "missing". The sweep ran from bench/2026-11-20-gemini4-neon-frontier-v4,
so its task hashes match the suite lock and no hash bridge is needed. Attempts
the AI Pro quota refused have no summary.json (they sit in infra-aborted/) and
are not part of the population.

When this runs from a worktree, link the sweep folder in first:
``ln -s ~/dev/VulcanBench/runs-effort-gemini4-neon-agy runs-effort-gemini4-neon-agy``.

    python scripts/cii-v4-board/build_neon_population.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

TASKS = ROOT / "tasks/coding-intelligence-index-v4"
OUTPUT = ROOT / "docs/results/swe-v4-gemini4-neon-2026-11/comparison.json"
MODELS = {"neon": ("runs-effort-gemini4-neon-agy", ("low", "medium", "high", "extra-high"))}
SWEEP_MODELS = {"neon": "agy:gemini-4-neon"}
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def main() -> None:
    rows, excluded, missing = [], [], []
    for model, (root, levels) in MODELS.items():
        for level in levels:
            seen = set()
            for run in sorted((ROOT / root / level).glob("legacy-*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if summary["model"] != SWEEP_MODELS[model]:
                    raise ValueError(f"{run}: unexpected solver {summary['model']}")
                seen.add(summary["task_id"])
                try:
                    data = base.inputs(run, TASKS)
                except ValueError as exc:
                    excluded.append(
                        {
                            "model": model,
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
                rows.append(
                    {
                        "model": model,
                        "effort": level,
                        "task": summary["task_id"],
                        "run_id": run.name,
                        "functional": scores["functional"],
                        "quality": scores["quality"],
                        "security": scores["security"],
                        "fallback": False,
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
                        {"model": model, "effort": level, "task": task, "reason": MISSING_REASON}
                    )
    cells = {}
    for r in rows:
        cells.setdefault(f"{r['model']}/{r['effort']}", 0)
        cells[f"{r['model']}/{r['effort']}"] += 1
    dupes = len(rows) - len({(r["model"], r["effort"], r["task"]) for r in rows})
    if dupes:
        raise ValueError(f"{dupes} duplicate model/effort/task rows")
    record = {
        "suite": "coding-intelligence-index-v4",
        "models": {"neon": "Gemini 4 Neon in Antigravity CLI"},
        "levels": {m: list(v[1]) for m, v in MODELS.items()},
        "cells": cells,
        "rows": rows,
        "excluded": excluded,
        "missing": missing,
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    print(f"{len(rows)} rows, {len(excluded)} excluded, {len(missing)} missing; cells: {cells}")
    for e in excluded:
        print("excluded:", e["model"], e["effort"], e["task"], e["reason"])
    for e in missing:
        print("missing:", e["model"], e["effort"], e["task"])


if __name__ == "__main__":
    main()

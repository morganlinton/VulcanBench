"""Build the Code quality judging population for the DeepSeek V4.1 Flash Frontier v4 sweep.

Reads every finished run under runs-effort-deepseek-flash-dsh/<level>/ (Low,
High and Max; DeepSeek's API has no medium) and writes the comparison record
the v3.27 runner freezes against, in the same row shape as the other Frontier v4
records. A run that cannot be reconstructed is listed under "excluded" with the
reason; a task and level with no finished run is "missing".

The sweep ran DeepSeek Harness 0.2.1-rc.2 (solver ``dsh:deepseek-flash``) from
the tagged run worktree bench/2026-10-20-deepseek-dsh-frontier-v4, so task hashes
match the suite lock. Runs interrupted by the owner's two pauses live in the
sweep's ``paused-interrupted/`` folder and are not read. The builder stops on
any run whose solver, requested effort, harness version or checkout state
differs from the sweep's record.

When this runs from a worktree, link the sweep folder in first:
``ln -s ~/dev/VulcanBench/runs-effort-deepseek-flash-dsh runs-effort-deepseek-flash-dsh``.

    python scripts/cii-v4-board/build_deepseek_flash_population.py
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
OUTPUT = ROOT / "docs/results/swe-v4-deepseek-flash-2026-10/comparison.json"
MODEL = "dsflash"
SWEEP_ROOT = "runs-effort-deepseek-flash-dsh"
LEVELS = ("low", "high", "max")
SOLVER = "dsh:deepseek-flash"
HARNESS_VERSION = "0.2.1-rc.2"
TASK_IDS = sorted(p.name for p in TASKS.iterdir() if p.is_dir() and p.name.startswith("legacy-"))
MISSING_REASON = "No finished run for this task and level"


def check_run(run: Path, summary: dict, level: str) -> None:
    """Refuse a run that does not belong to the sweep as recorded."""
    if summary["model"] != SOLVER:
        raise ValueError(f"{run}: unexpected solver {summary['model']}")
    if (summary.get("effort") or {}).get("requested") != level:
        raise ValueError(f"{run}: requested effort is not {level}")
    if summary["cli_agent"]["harness_version"] != HARNESS_VERSION:
        raise ValueError(f"{run}: harness version {summary['cli_agent']['harness_version']}")
    if summary["source"]["dirty"] or summary["source"]["allow_dirty"]:
        raise ValueError(f"{run}: not run from a clean checkout")


def main() -> None:
    rows, excluded, missing = [], [], []
    for level in LEVELS:
        seen = set()
        for run in sorted((ROOT / SWEEP_ROOT / level).glob("legacy-*/")):
            summary_path = run / "summary.json"
            if not summary_path.exists():
                continue
            summary = json.loads(summary_path.read_text())
            check_run(run, summary, level)
            if summary["task_id"] in seen:
                raise ValueError(f"{run}: second run of {summary['task_id']} at {level}")
            seen.add(summary["task_id"])
            try:
                data = base.inputs(run, TASKS)
            except ValueError as exc:
                excluded.append(
                    {
                        "model": MODEL,
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
                    "model": MODEL,
                    "effort": level,
                    "task": summary["task_id"],
                    "run_id": run.name,
                    "functional": scores["functional"],
                    "quality": scores["quality"],
                    "security": scores["security"],
                    "duration_s": summary["duration_s"],
                    "reported_tokens": summary["total_tokens"],
                    "solver_receipt": solver_receipt(run, summary),
                    "source_directory": str(run),
                    "source_hashes": data["source_hashes"],
                    "started_at": summary["started_at"],
                    "finished_at": summary["finished_at"],
                    "solver_cli_version": summary["cli_agent"]["harness_version"],
                    "api_equivalent_cost_usd": summary["economics"]["api_equivalent_cost_usd"],
                    "billed_cost_usd": summary["cli_agent"]["cli_reported_cost_usd"],
                }
            )
        for task in TASK_IDS:
            if task not in seen:
                missing.append(
                    {"model": MODEL, "effort": level, "task": task, "reason": MISSING_REASON}
                )
    cells: dict[str, int] = {}
    for r in rows:
        cells[f"{r['model']}/{r['effort']}"] = cells.get(f"{r['model']}/{r['effort']}", 0) + 1
    record = {
        "suite": "coding-intelligence-index-v4",
        "models": {
            MODEL: f"DeepSeek V4.1 Flash (deepseek-flash) in DeepSeek Harness {HARNESS_VERSION}"
        },
        "levels": {MODEL: list(LEVELS)},
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

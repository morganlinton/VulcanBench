"""Build the judging population for the private Routine v1 sweeps.

Reads every finished run under ``VulcanRoutine/runs/runs-routine-v1-<model>/
<level>/`` and writes the comparison record the Code quality v3.8 runner
freezes against, in the same row shape as the Frontier v4 records. Unfinished
runs are listed under "excluded" with the reason; a task and level with no
attempt at all is listed under "missing". When a resumed sweep left two
finished runs for one task and level, the latest is kept and the other is
listed under "duplicates".

The record names private tasks, so it is written inside the private
repository (``results/private/routine-v1-comparison.json``), never here.

    python scripts/cii-v4-board/build_routine_population.py            # v3.8 population
    python scripts/cii-v4-board/build_routine_population.py --opus55   # v3.14 population
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from harness import retrospective_judging as base  # noqa: E402
from harness.solver_receipts import solver_receipt  # noqa: E402

ROUTINE = Path(os.environ.get("VULCANROUTINE_ROOT", ROOT.parent / "VulcanRoutine")).resolve()
TASKS = ROUTINE / "tasks/routine-v1"
OUTPUT = ROUTINE / "results/private/routine-v1-comparison.json"
ALL_LEVELS = tuple(base.LEVELS)
# model key -> (run root under VulcanRoutine/runs, levels, solver spec, display name)
MODELS = {
    "astra": ("runs-routine-v1-astra", ALL_LEVELS, "codex:gpt-6-astra", "GPT-6 Astra in Codex"),
    "terra": ("runs-routine-v1-terra", ALL_LEVELS, "codex:gpt-5.6-terra", "GPT-5.6 Terra in Codex"),
    "luna": ("runs-routine-v1-luna", ALL_LEVELS, "codex:gpt-5.6-luna", "GPT-5.6 Luna in Codex"),
    "sol": ("runs-routine-v1-sol", ALL_LEVELS, "codex:gpt-5.6-sol", "GPT-5.6 Sol in Codex"),
    "gpt55": (
        "runs-routine-v1-gpt55",
        ("low", "medium", "high", "extra-high"),
        "codex:gpt-5.5",
        "GPT-5.5 in Codex",
    ),
    "fable": (
        "runs-routine-v1-fable",
        ALL_LEVELS,
        "claude-code:claude-fable-5-1",
        "Claude Fable 5.1 in Claude Code",
    ),
    "swe2": (
        "runs-routine-v1-swe2",
        ("medium", "high", "max"),
        "devin:swe-2",
        "SWE-2 in Devin CLI",
    ),
}
# Claude Opus 5.5 is judged under its own protocol (v3.14), not added to v3.8:
# v3.8 pins the SHA-256 of OUTPUT, so rebuilding that file with a new model
# would break verification of the published record. ``--opus55`` writes a
# separate comparison record for v3.14 and leaves OUTPUT untouched.
OPUS55_MODELS = {
    "opus55": (
        "runs-routine-v1-opus55",
        ALL_LEVELS,
        "claude-code:claude-opus-5-5",
        "Claude Opus 5.5 in Claude Code",
    ),
}
OPUS55_OUTPUT = ROUTINE / "results/private/routine-v1-comparison-opus55.json"
MISSING_REASON = "No finished run for this task and level"


def task_ids(tasks_root: Path) -> list[str]:
    return sorted(
        p.name for p in tasks_root.iterdir() if p.is_dir() and (p / "metadata.json").exists()
    )


def _receipt(run: Path, summary: dict) -> dict:
    """The solver's raw usage receipt; a receipt problem never blocks Code quality judging."""
    try:
        return solver_receipt(run, summary)
    except (ValueError, KeyError, OSError) as exc:
        return {"error": f"{type(exc).__name__}: {exc}"}


def build(models: dict, runs_root: Path, tasks_root: Path) -> dict:  # noqa: PLR0912, one linear scan
    rows, excluded, missing, duplicates = [], [], [], []
    all_tasks = task_ids(tasks_root)
    for model, (root, levels, spec, _name) in models.items():
        for level in levels:
            best: dict[str, dict] = {}
            attempted = set()
            for run in sorted((runs_root / root / level).glob("*/")):
                summary_path = run / "summary.json"
                if not summary_path.exists():
                    continue
                summary = json.loads(summary_path.read_text())
                if summary["model"] != spec:
                    raise ValueError(f"{run}: unexpected solver {summary['model']}")
                task = summary["task_id"]
                attempted.add(task)
                try:
                    data = base.inputs(run, tasks_root)
                except ValueError as exc:
                    excluded.append(
                        {
                            "model": model,
                            "effort": level,
                            "task": task,
                            "run_id": run.name,
                            "reason": str(exc),
                            "finished": summary.get("finished"),
                            "duration_s": summary["duration_s"],
                            "functional": summary["scores"]["functional"],
                        }
                    )
                    continue
                scores = summary["scores"]
                row = {
                    "model": model,
                    "effort": level,
                    "task": task,
                    "run_id": run.name,
                    "functional": scores["functional"],
                    "quality": scores["quality"],
                    "security": scores["security"],
                    "fallback": False,
                    "duration_s": summary["duration_s"],
                    "reported_tokens": summary["total_tokens"],
                    "solver_receipt": _receipt(run, summary),
                    "source_directory": str(run),
                    "source_hashes": data["source_hashes"],
                    "started_at": summary["started_at"],
                    "finished_at": summary["finished_at"],
                    "solver_cli_version": summary["cli_agent"]["harness_version"],
                    "api_equivalent_cost_usd": summary["economics"]["api_equivalent_cost_usd"],
                }
                previous = best.get(task)
                if previous is None or row["finished_at"] > previous["finished_at"]:
                    if previous is not None:
                        duplicates.append(
                            {k: previous[k] for k in ("model", "effort", "task", "run_id")}
                        )
                    best[task] = row
                else:
                    duplicates.append({k: row[k] for k in ("model", "effort", "task", "run_id")})
            rows.extend(best[t] for t in sorted(best))
            # A task whose only attempts were excluded is already accounted for above.
            excluded = [
                e
                for e in excluded
                if not (e["model"] == model and e["effort"] == level and e["task"] in best)
            ]
            for task in all_tasks:
                if task not in attempted:
                    missing.append(
                        {"model": model, "effort": level, "task": task, "reason": MISSING_REASON}
                    )
    cells: dict[str, int] = {}
    for r in rows:
        cells[f"{r['model']}/{r['effort']}"] = cells.get(f"{r['model']}/{r['effort']}", 0) + 1
    # One excluded entry per task and level, so cell arithmetic stays rows + gaps = tasks.
    seen, unique_excluded = set(), []
    for e in excluded:
        cell = (e["model"], e["effort"], e["task"])
        if cell not in seen:
            seen.add(cell)
            unique_excluded.append(e)
    return {
        "suite": "routine-v1",
        "models": {m: v[3] for m, v in models.items()},
        "levels": {m: list(v[1]) for m, v in models.items()},
        "cells": cells,
        "rows": rows,
        "excluded": unique_excluded,
        "missing": missing,
        "duplicates": duplicates,
    }


def main() -> None:
    models, output = (OPUS55_MODELS, OPUS55_OUTPUT) if "--opus55" in sys.argv else (MODELS, OUTPUT)
    record = build(models, ROUTINE / "runs", TASKS)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    print(
        f"{len(record['rows'])} rows, {len(record['excluded'])} excluded, {len(record['missing'])} missing, "
        f"{len(record['duplicates'])} duplicates; cells: {record['cells']}"
    )
    bad = [r for r in record["rows"] if "error" in r["solver_receipt"]]
    if bad:
        print(
            f"{len(bad)} rows carry a solver receipt error (economics only; judging is unaffected)"
        )


if __name__ == "__main__":
    main()

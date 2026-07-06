"""Assemble a model x effort comparison from cached runs, re-running nothing.

Grading is deterministic and every run records the ``task_hash`` it was scored
against, so a comparison across models does not require re-running baselines: it
is a query over ``./runs``. This module builds that query.

A suite is "frozen" implicitly by its task hashes. ``suite_version`` summarizes
the current task set as a single short hash; any cached run whose recorded
``task_hash`` no longer matches its task is *stale* and excluded, so a comparison
only ever mixes results from the same frozen definition. Adding a model to a
report is therefore: run that one model, then rebuild the matrix from cache. The
baselines are never paid for twice.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from harness.effort import normalize_effort
from harness.leaderboard import current_task_hashes, mark_stale, summary_to_row
from harness.suite import DEFAULT_TASKS_BASE, load_suite


def _scan_runs_recursive(runs_dir: Path) -> list[dict[str, Any]]:
    """One leaderboard row per run under ``runs_dir`` at any nesting depth.

    ``harness.leaderboard.load_summaries`` only looks one level deep; sweeps and
    experiments nest run dirs under a sub-directory, so compare discovers
    ``summary.json`` recursively.
    """
    rows: list[dict[str, Any]] = []
    if not runs_dir.exists():
        return rows
    for sj in sorted(runs_dir.glob("**/summary.json")):
        try:
            s = json.loads(sj.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        s.setdefault("run_id", sj.parent.name)
        rows.append(summary_to_row(s, fallback_run_id=sj.parent.name))
    return rows


def suite_version(suite: str, tasks_base: Path = DEFAULT_TASKS_BASE) -> dict[str, Any]:
    """Return the frozen identity of a suite: its task set and a version hash.

    The version is a hash of the sorted ``(task_id, task_hash)`` pairs, so it
    changes exactly when the suite's task set or any task's scoring definition
    changes. Two runs sharing a suite version are directly comparable.
    """
    s = load_suite(suite, tasks_base)
    hashes = current_task_hashes(s.tasks_root)
    pairs = sorted((tid, hashes.get(tid, "?")) for tid in s.task_ids)
    digest = hashlib.sha256("\n".join(f"{tid}:{h}" for tid, h in pairs).encode()).hexdigest()
    return {
        "suite": suite,
        "version": digest[:12],
        "n_tasks": len(s.task_ids),
        "task_ids": list(s.task_ids),
        "task_hashes": {tid: h for tid, h in pairs},
    }


def _cell_key(row: dict[str, Any]) -> tuple[str, str]:
    return (row.get("model") or "?", row.get("effort_requested") or "-")


def fresh_run_counts(
    task_ids: list[str],
    model: str,
    effort: str | None,
    runs_dir: Path,
    tasks_root: Path,
) -> dict[str, int]:
    """Count non-stale cached runs per task for one (model, effort) cell.

    A run counts only if its task still matches the current definition on disk
    (its recorded ``task_hash`` is fresh). Used by the suite runner to skip tasks
    that already have enough cached runs, so ``--only-missing`` fills just the
    gaps instead of re-running a whole column.
    """
    want_effort = normalize_effort(effort)
    wanted = set(task_ids)
    rows = [
        r
        for r in _scan_runs_recursive(runs_dir)
        if r.get("task_id") in wanted
        and r.get("model") == model
        and normalize_effort(r.get("effort_requested")) == want_effort
    ]
    mark_stale(rows, tasks_root)
    counts: dict[str, int] = dict.fromkeys(task_ids, 0)
    for r in rows:
        if r.get("task_stale") is not True:
            counts[str(r.get("task_id"))] += 1
    return counts


def build_matrix(
    suite: str,
    runs_dir: Path = Path("./runs"),
    tasks_base: Path = DEFAULT_TASKS_BASE,
) -> dict[str, Any]:
    """Build the model x effort comparison for ``suite`` from cached runs.

    Each cell aggregates the non-stale cached runs for one (model, effort) over
    the suite's tasks: pass@1 (mean over tasks of the per-task solve rate), total
    cached cost, and coverage (how many of the suite's tasks have a fresh run). A
    cell is ``complete`` when every suite task is covered. Incomplete cells list
    the tasks still to run — those, and only those, need new model calls.
    """
    ver = suite_version(suite, tasks_base)
    suite_tasks = set(ver["task_ids"])
    s = load_suite(suite, tasks_base)

    rows = [r for r in _scan_runs_recursive(runs_dir) if r.get("task_id") in suite_tasks]
    mark_stale(rows, s.tasks_root)
    fresh = [r for r in rows if r.get("task_stale") is not True]
    n_stale_excluded = sum(1 for r in rows if r.get("task_stale") is True)

    # Group fresh runs into (model, effort) cells, then by task within a cell.
    cells: dict[tuple[str, str], dict[str, list[dict[str, Any]]]] = {}
    for r in fresh:
        cells.setdefault(_cell_key(r), {}).setdefault(str(r.get("task_id")), []).append(r)

    cell_records: list[dict[str, Any]] = []
    for (model, effort), by_task in sorted(cells.items()):
        present = [t for t in suite_tasks if t in by_task]
        # pass@1: mean over covered tasks of the per-task solve rate.
        solve_rates = []
        cost = 0.0
        cost_known = True
        for t in present:
            runs = by_task[t]
            solved = sum(1 for x in runs if (x.get("functional") or 0) >= 1.0)
            solve_rates.append(solved / len(runs))
            for x in runs:
                c = x.get("cost_usd")
                if c is None:
                    cost_known = False
                else:
                    cost += c
        missing = sorted(suite_tasks - set(present))
        cell_records.append(
            {
                "model": model,
                "effort": effort,
                "n_present": len(present),
                "n_total": len(suite_tasks),
                "complete": len(present) == len(suite_tasks),
                "pass_at_1": round(sum(solve_rates) / len(solve_rates), 4) if solve_rates else None,
                "cost_usd": round(cost, 4) if cost_known else None,
                "missing_tasks": missing,
            }
        )

    cell_records.sort(key=lambda c: (-(c["pass_at_1"] or -1), c["model"], c["effort"]))
    return {
        **ver,
        "runs_dir": str(runs_dir),
        "n_stale_excluded": n_stale_excluded,
        "cells": cell_records,
    }

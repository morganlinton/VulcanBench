"""Leaderboard aggregation over local ``./runs/`` summaries.

``scan_leaderboard`` returns one row per run; ``aggregate_by_model`` rolls those
up into a per-model ranking with statistics that survive repeated runs:

- **pass@1**: expected single-attempt solve rate: for each task, the fraction of
  that task's attempts that scored ``functional == 1.0``, averaged over tasks.
- **pass@k**: solved at least once: for each task, 1 if any attempt solved it,
  averaged over tasks (``k`` = attempts per task).
- **stderr**: standard error across tasks (0 with fewer than two tasks).

Reporting a single noisy run per model is misleading; running each task ``N``
times (``--repeat N``) and reporting pass@1 ± stderr / pass@k makes the
leaderboard meaningful.
"""

from __future__ import annotations

import json
import math
import statistics
from collections import defaultdict
from pathlib import Path
from typing import Any

from harness.task_metadata import repo_scale
from harness.tasks import list_task_ids, load_task, task_hash

_METRIC_KEYS = ("functional", "quality", "security", "efficiency", "human_like")


def track_for_run(cli_agent: dict[str, Any] | None) -> str:
    """API vs subscription track. Pi is a CLI harness that still bills API."""
    if not cli_agent:
        return "api"
    return "api" if cli_agent.get("billing") == "api" else "subscription"


def summary_to_row(s: dict[str, Any], fallback_run_id: str = "") -> dict[str, Any]:
    """Project a run summary onto a compact leaderboard row."""
    scores = s.get("scores", {})
    manifest_task = (s.get("manifest") or {}).get("task", {})
    effort = s.get("effort")
    cli_agent = s.get("cli_agent") or {}
    economics = s.get("economics") or {}
    execution_harness = cli_agent.get("harness") or "vulcan"
    track = track_for_run(cli_agent)
    legacy_cost = s.get("cost_usd")
    marginal_cash = economics.get("marginal_cash_usd")
    api_equivalent = economics.get("api_equivalent_cost_usd")
    if not economics and not cli_agent:
        marginal_cash = legacy_cost
        api_equivalent = legacy_cost
    elif not economics and cli_agent:
        api_equivalent = legacy_cost
    return {
        "run_id": s.get("run_id", fallback_run_id),
        "task_id": s.get("task_id"),
        "model": s.get("model"),
        "total": scores.get("total", scores.get("functional", 0)),
        "functional": scores.get("functional"),
        "quality": scores.get("quality"),
        "security": scores.get("security"),
        "efficiency": scores.get("efficiency"),
        "human_like": scores.get("human_like"),
        "steps": s.get("steps"),
        "total_tokens": s.get("total_tokens"),
        "cost_usd": s.get("cost_usd"),
        "execution_harness": execution_harness,
        "track": track,
        "access_mode": economics.get("billing_mode")
        or ("api-metered" if track == "api" else "subscription-included"),
        "cost_basis": economics.get("cost_basis"),
        "marginal_cash_usd": marginal_cash,
        "api_equivalent_cost_usd": api_equivalent,
        "allocated_plan_cost_usd": economics.get("allocated_plan_cost_usd"),
        "plan_name": economics.get("plan_name") or cli_agent.get("plan_name"),
        "reported_model": cli_agent.get("reported_model"),
        "harness_version": cli_agent.get("harness_version"),
        "duration_s": s.get("duration_s"),
        "suite": s.get("suite"),
        "suite_id": s.get("suite_id"),
        "effort": effort,
        "effort_requested": effort.get("requested") if isinstance(effort, dict) else None,
        "experiment_id": s.get("experiment_id"),
        "task_hash": s.get("task_hash"),
        "finished_at": s.get("finished_at"),
        "repo_scale": manifest_task.get("repo_scale"),
        "task_complexity": manifest_task.get("task_complexity"),
        "languages": manifest_task.get("languages"),
        "difficulty": manifest_task.get("difficulty"),
    }


def load_summaries(runs_dir: Path = Path("./runs")) -> list[dict[str, Any]]:
    """Load every run summary under ``runs_dir`` (full dicts, including manifest)."""
    out: list[dict[str, Any]] = []
    if not runs_dir.exists():
        return out
    for d in sorted(runs_dir.iterdir()):
        sj = d / "summary.json"
        if not (d.is_dir() and sj.exists()):
            continue
        try:
            s = json.loads(sj.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        s.setdefault("run_id", d.name)
        out.append(s)
    return out


def scan_leaderboard(runs_dir: Path = Path("./runs")) -> list[dict[str, Any]]:
    """One row per run, with scores, cost, latency, and suite tag (when present)."""
    return [
        summary_to_row(s, fallback_run_id=s.get("run_id", "")) for s in load_summaries(runs_dir)
    ]


def current_task_hashes(tasks_root: Path = Path("tasks/v1")) -> dict[str, str]:
    """Map task_id -> current scoring-definition hash for the tasks on disk."""
    hashes: dict[str, str] = {}
    for task_id in list_task_ids(tasks_root):
        try:
            hashes[task_id] = task_hash(load_task(task_id, tasks_root))
        except (OSError, ValueError):
            continue
    return hashes


def mark_stale(
    rows: list[dict[str, Any]], tasks_root: Path = Path("tasks/v1")
) -> list[dict[str, Any]]:
    """Annotate each row with ``task_stale``: True if it was scored against a task
    version whose hash no longer matches the current definition on disk.

    Rows whose task no longer exists, or whose run predates hashing (no recorded
    ``task_hash``), are left ``task_stale=None`` (unknown) rather than flagged.
    """
    current = current_task_hashes(tasks_root)
    for r in rows:
        recorded = r.get("task_hash")
        tid = r.get("task_id")
        expected = current.get(tid) if isinstance(tid, str) else None
        if not recorded or expected is None:
            r["task_stale"] = None
        else:
            r["task_stale"] = recorded != expected
    return rows


def _is_solved(row: dict[str, Any]) -> bool:
    return (row.get("functional") or 0) >= 1.0


def filter_rows_by_repo_scale(
    rows: list[dict[str, Any]], scales: set[str], tasks_root: Path = Path("tasks/v1")
) -> list[dict[str, Any]]:
    """Keep rows whose task ``repo_scale`` is in ``scales`` (loads metadata when missing)."""
    out: list[dict[str, Any]] = []
    for r in rows:
        scale = r.get("repo_scale")
        tid = r.get("task_id")
        if scale is None and isinstance(tid, str):
            try:
                scale = repo_scale(load_task(tid, tasks_root).metadata)
            except (OSError, ValueError):
                scale = None
        if scale in scales:
            out.append(r)
    return out


def aggregate_by_model(
    rows: list[dict[str, Any]],
    suite: str | None = None,
    repo_scale_filter: set[str] | None = None,
    tasks_root: Path = Path("tasks/v1"),
) -> list[dict[str, Any]]:
    """Roll per-run rows up into a per-model ranking, sorted by pass@1 desc.

    Repeated attempts of the same task are grouped so pass@1 / pass@k / stderr
    are computed per task and then averaged over tasks.
    """
    if suite is not None:
        rows = [r for r in rows if r.get("suite") == suite]
    if repo_scale_filter is not None:
        rows = filter_rows_by_repo_scale(rows, repo_scale_filter, tasks_root)

    by_model: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_model[r.get("model") or "?"].append(r)

    aggregates: list[dict[str, Any]] = []
    for model, model_rows in by_model.items():
        by_task: dict[Any, list[dict[str, Any]]] = defaultdict(list)
        for r in model_rows:
            by_task[r.get("task_id")].append(r)

        per_task_rate: list[float] = []  # fraction of attempts solved, per task
        per_task_any: list[float] = []  # solved at least once (0/1), per task
        for attempts in by_task.values():
            solved = sum(1 for a in attempts if _is_solved(a))
            per_task_rate.append(solved / len(attempts))
            per_task_any.append(1.0 if solved else 0.0)

        n_runs = len(model_rows)
        n_tasks = len(by_task)
        costs = [r["cost_usd"] for r in model_rows if r.get("cost_usd") is not None]
        marginal_costs = [
            r["marginal_cash_usd"] for r in model_rows if r.get("marginal_cash_usd") is not None
        ]
        api_equivalent_costs = [
            r["api_equivalent_cost_usd"]
            for r in model_rows
            if r.get("api_equivalent_cost_usd") is not None
        ]
        durations = [r["duration_s"] for r in model_rows if r.get("duration_s") is not None]
        harnesses = {str(r.get("execution_harness") or "vulcan") for r in model_rows}
        tracks = {str(r.get("track") or "api") for r in model_rows}
        access_modes = {str(r.get("access_mode") or "unknown") for r in model_rows}

        agg: dict[str, Any] = {
            "model": model,
            "execution_harness": next(iter(harnesses)) if len(harnesses) == 1 else "mixed",
            "track": next(iter(tracks)) if len(tracks) == 1 else "mixed",
            "access_mode": next(iter(access_modes)) if len(access_modes) == 1 else "mixed",
            "n_tasks": n_tasks,
            "n_runs": n_runs,
            "repeats": round(n_runs / n_tasks, 2) if n_tasks else 0,
            "solved": sum(1 for r in model_rows if _is_solved(r)),
            "pass_at_1": _mean(per_task_rate),
            "pass_at_1_stderr": _stderr(per_task_rate),
            "pass_at_k": _mean(per_task_any),
            "avg_total": _mean([r.get("total") for r in model_rows]),
            "avg_total_stderr": _stderr([r.get("total") for r in model_rows]),
            "total_tokens": sum(r.get("total_tokens") or 0 for r in model_rows),
            "total_cost": round(sum(costs), 6) if costs else None,
            "cost_known": len(costs) == n_runs,
            "total_marginal_cash_usd": (round(sum(marginal_costs), 6) if marginal_costs else None),
            "marginal_cash_known": len(marginal_costs) == n_runs,
            "total_api_equivalent_cost_usd": (
                round(sum(api_equivalent_costs), 6) if api_equivalent_costs else None
            ),
            "api_equivalent_known": len(api_equivalent_costs) == n_runs,
            "avg_duration_s": _mean(durations),
        }
        for key in _METRIC_KEYS:
            agg[f"avg_{key}"] = _mean([r.get(key) for r in model_rows])
        aggregates.append(agg)

    aggregates.sort(
        key=lambda a: (
            a["pass_at_1"] if a["pass_at_1"] is not None else -1,
            a["avg_total"] if a["avg_total"] is not None else -1,
        ),
        reverse=True,
    )
    return aggregates


def _mean(values: Any) -> float | None:
    nums = [v for v in values if isinstance(v, (int, float))]
    return round(sum(nums) / len(nums), 4) if nums else None


def _stderr(values: Any) -> float:
    """Standard error of the mean across a sample (0 with fewer than two points)."""
    nums = [float(v) for v in values if isinstance(v, (int, float))]
    if len(nums) < 2:
        return 0.0
    return round(float(statistics.stdev(nums)) / math.sqrt(len(nums)), 4)

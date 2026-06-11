"""Build a shareable results report from recorded runs.

``build_report`` produces a structured dict (JSON-serializable); ``to_markdown``
renders it for humans. A report has:

- ``models``      — the per-model ranking (pass@1 ± stderr, pass@k, cost, latency)
- ``tasks``       — per-task breakdown (per-model attempts / solve rate)
- ``environment`` — distinct models / Python / tool versions seen in run manifests
- ``integrity``   — runs scored against a now-stale task version, and runs
  scored against tasks not known to be decontaminated (``decontaminated: false``)
- ``totals``      — run / model / task counts and total known cost
"""

from __future__ import annotations

import json
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.calibration import calibrate_tasks, calibration_to_markdown
from harness.leaderboard import (
    aggregate_by_model,
    load_summaries,
    mark_stale,
    summary_to_row,
)


def build_report(
    runs_dir: Path = Path("./runs"),
    tasks_root: Path = Path("tasks/v1"),
    suite: str | None = None,
    generated_at: str | None = None,
) -> dict[str, Any]:
    """Assemble a report dict from the run summaries under ``runs_dir``."""
    summaries = load_summaries(runs_dir)
    if suite is not None:
        summaries = [s for s in summaries if s.get("suite") == suite]
    rows = [summary_to_row(s, s.get("run_id", "")) for s in summaries]
    mark_stale(rows, tasks_root)

    models = aggregate_by_model(rows)
    tasks = _per_task(rows)
    environment = _environment(summaries)
    integrity = _integrity(rows, tasks_root)
    calibration = calibrate_tasks(rows, tasks_root)
    known_costs = [r["cost_usd"] for r in rows if r.get("cost_usd") is not None]

    return {
        "generated_at": generated_at or datetime.now(UTC).isoformat(),
        "suite": suite,
        "totals": {
            "n_runs": len(rows),
            "n_models": len(models),
            "n_tasks": len({r.get("task_id") for r in rows}),
            "total_cost_usd": round(sum(known_costs), 6) if known_costs else None,
        },
        "models": models,
        "tasks": tasks,
        "environment": environment,
        "integrity": integrity,
        "calibration": calibration,
    }


def _per_task(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_task: dict[Any, list[dict[str, Any]]] = defaultdict(list)
    for r in rows:
        by_task[r.get("task_id")].append(r)
    out: list[dict[str, Any]] = []
    for task_id in sorted(by_task, key=str):
        attempts = by_task[task_id]
        by_model: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for a in attempts:
            by_model[a.get("model") or "?"].append(a)
        model_stats = []
        for model in sorted(by_model):
            ms = by_model[model]
            solved = sum(1 for a in ms if (a.get("functional") or 0) >= 1.0)
            model_stats.append(
                {
                    "model": model,
                    "attempts": len(ms),
                    "solved": solved,
                    "solve_rate": round(solved / len(ms), 4),
                }
            )
        out.append({"task_id": task_id, "n_runs": len(attempts), "models": model_stats})
    return out


def _environment(summaries: list[dict[str, Any]]) -> dict[str, Any]:
    models: set[str] = set()
    pythons: set[str] = set()
    tools: dict[str, set[str]] = defaultdict(set)
    for s in summaries:
        manifest = s.get("manifest") or {}
        if manifest.get("model"):
            models.add(str(manifest["model"]))
        runtime = manifest.get("runtime") or {}
        if runtime.get("python"):
            pythons.add(str(runtime["python"]))
        for tool, version in (manifest.get("tools") or {}).items():
            if version:
                tools[tool].add(str(version))
    return {
        "models": sorted(models),
        "python": sorted(pythons),
        "tools": {t: sorted(v) for t, v in sorted(tools.items())},
    }


def _not_decontaminated_tasks(tasks_root: Path, task_ids: set[Any]) -> set[str]:
    """Task ids whose metadata explicitly marks them not decontaminated.

    Only ``decontaminated: false`` counts — a missing field is treated as
    unknown here (the validator is what *requires* the field at authoring time).
    """
    flagged: set[str] = set()
    for tid in task_ids:
        if not tid:
            continue
        meta_path = tasks_root / str(tid) / "metadata.json"
        if not meta_path.exists():
            continue
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if meta.get("decontaminated") is False:
            flagged.add(str(tid))
    return flagged


def _integrity(rows: list[dict[str, Any]], tasks_root: Path) -> dict[str, Any]:
    stale = [r["run_id"] for r in rows if r.get("task_stale") is True]
    unknown = sum(1 for r in rows if r.get("task_stale") is None)
    flagged = _not_decontaminated_tasks(tasks_root, {r.get("task_id") for r in rows})
    nd_runs = [r["run_id"] for r in rows if str(r.get("task_id")) in flagged]
    return {
        "stale": len(stale),
        "stale_run_ids": stale,
        "unknown": unknown,
        "not_decontaminated": len(nd_runs),
        "not_decontaminated_run_ids": nd_runs,
        "not_decontaminated_tasks": sorted(flagged),
    }


def _fmt(n: Any) -> str:
    return "—" if n is None else (f"{n:.4f}" if isinstance(n, float) else str(n))


def to_markdown(report: dict[str, Any]) -> str:
    suite = report.get("suite") or "all runs"
    totals = report["totals"]
    lines: list[str] = [
        f"# VulcanBench report — {suite}",
        "",
        f"_Generated {report['generated_at']}_",
        "",
        f"- **{totals['n_runs']}** runs · **{totals['n_models']}** models · "
        f"**{totals['n_tasks']}** tasks · total cost "
        f"{('$' + str(totals['total_cost_usd'])) if totals['total_cost_usd'] is not None else 'n/a'}",
    ]

    integ = report["integrity"]
    if integ["stale"]:
        lines += [
            "",
            f"> ⚠️ **{integ['stale']} run(s) scored against a now-stale task version** "
            f"(task definition changed since): {', '.join(integ['stale_run_ids'])}",
        ]
    if integ.get("not_decontaminated"):
        lines += [
            "",
            f"> ⚠️ **{integ['not_decontaminated']} run(s) scored against "
            f"non-decontaminated task(s)** "
            f"({', '.join(integ['not_decontaminated_tasks'])}) — these tasks derive from "
            "public sources that predate model training cutoffs; treat their scores with care.",
        ]

    lines += [
        "",
        "## Models",
        "",
        "| Model | Tasks | Runs | pass@1 ± se | pass@k | Avg total | Cost $ | Avg time |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for m in report["models"]:
        cost = "?" if not m.get("cost_known") else _fmt(m.get("total_cost"))
        lines.append(
            f"| {m['model']} | {m['n_tasks']} | {m['n_runs']} | "
            f"{_fmt(m['pass_at_1'])} ± {_fmt(m['pass_at_1_stderr'])} | {_fmt(m['pass_at_k'])} | "
            f"{_fmt(m['avg_total'])} | {cost} | {_fmt(m['avg_duration_s'])} |"
        )

    lines += ["", "## Per-task", "", "| Task | Runs | Model | Solve rate |", "|---|---|---|---|"]
    for t in report["tasks"]:
        for ms in t["models"]:
            lines.append(
                f"| {t['task_id']} | {ms['attempts']} | {ms['model']} | "
                f"{ms['solved']}/{ms['attempts']} ({_fmt(ms['solve_rate'])}) |"
            )

    env = report["environment"]
    lines += ["", "## Environment", ""]
    lines.append(f"- Models: {', '.join(env['models']) or '—'}")
    lines.append(f"- Python: {', '.join(env['python']) or '—'}")
    for tool, versions in env["tools"].items():
        lines.append(f"- {tool}: {', '.join(versions)}")

    cal = report.get("calibration")
    if cal and cal["summary"]["n_calibrated"]:
        lines.append("")
        lines.append(calibration_to_markdown(cal))

    return "\n".join(lines) + "\n"

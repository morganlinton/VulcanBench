"""VulcanBench API.

Reads run data either from a database (when ``DATABASE_URL`` is set; see
``backend.db``) or, by default, by scanning the local ``./runs/`` directory.
Trace/patch/replay artifacts are always served from the filesystem.

Run locally::

    uvicorn backend.app:app --reload --port 8000
"""

from __future__ import annotations

import json
import os
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any, cast

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from backend import db
from harness.calibration import calibrate_tasks
from harness.leaderboard import aggregate_by_model, filter_rows_by_repo_scale, scan_leaderboard
from harness.report import build_effort_sensitivity
from harness.suite import SUITE_ALIASES, load_suite
from harness.tasks import list_task_ids, load_task

TASKS_ROOT = Path(os.environ.get("VULCANBENCH_TASKS_ROOT", "tasks/v1"))

RUNS_DIR = Path(os.environ.get("VULCANBENCH_RUNS_DIR", "./runs"))


@asynccontextmanager
async def _lifespan(_app: FastAPI) -> AsyncIterator[None]:
    db.init_db()  # no-op unless DATABASE_URL is set
    try:
        yield
    finally:
        db.close_db()


app = FastAPI(title="VulcanBench API", version="0.1.0", lifespan=_lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


def _rows() -> list[dict[str, Any]]:
    """Per-run rows from the database when configured, else the filesystem."""
    return db.leaderboard_rows() if db.enabled() else scan_leaderboard(RUNS_DIR)


class FeedbackIn(BaseModel):
    task_id: str | None = None
    run_id: str | None = None
    rating: int | None = None
    comment: str | None = None


@app.get("/api/health")
def health() -> dict[str, str]:
    return {"status": "ok", "store": "db" if db.enabled() else "filesystem"}


@app.post("/api/runs")
def post_run(summary: dict[str, Any]) -> dict[str, Any]:
    """Record a run summary (database only)."""
    if not db.enabled():
        raise HTTPException(status_code=400, detail="no database configured (set DATABASE_URL)")
    if not summary.get("run_id"):
        raise HTTPException(status_code=422, detail="summary.run_id is required")
    db.upsert_run(summary)
    return {"ok": True, "run_id": summary["run_id"]}


@app.post("/api/feedback")
def post_feedback(fb: FeedbackIn) -> dict[str, Any]:
    """Record task/run feedback (database only)."""
    if not db.enabled():
        raise HTTPException(status_code=400, detail="no database configured (set DATABASE_URL)")
    return db.add_feedback(fb.task_id, fb.run_id, fb.rating, fb.comment)


@app.get("/api/leaderboard")
def get_leaderboard(
    by: str = "model",
    task: str | None = None,
    suite: str | None = None,
    repo_scale: str | None = None,
    effort: str | None = None,
    task_complexity: str | None = None,
) -> list[dict[str, Any]]:
    rows = _rows()
    if suite in SUITE_ALIASES:
        allowed = set(load_suite(suite).task_ids)
        rows = [r for r in rows if r.get("task_id") in allowed]
    elif suite:
        rows = [r for r in rows if r.get("suite") == suite]
    if repo_scale:
        scales = {s.strip() for s in repo_scale.split(",") if s.strip()}
        rows = filter_rows_by_repo_scale(rows, scales, TASKS_ROOT)
    if effort:
        efforts = {e.strip() for e in effort.split(",") if e.strip()}
        rows = [r for r in rows if r.get("effort_requested") in efforts]
    if task_complexity:
        complexities = {c.strip() for c in task_complexity.split(",") if c.strip()}
        rows = [r for r in rows if r.get("task_complexity") in complexities]
    if by == "model":
        return aggregate_by_model(rows, suite=None)
    if task:
        rows = [r for r in rows if r.get("task_id") == task]
    rows.sort(key=lambda r: r.get("total") or 0, reverse=True)
    return rows


@app.get("/api/run/{run_id}")
def get_run(run_id: str) -> dict[str, Any]:
    if db.enabled():
        summary = db.get_summary(run_id)
        if summary is None:
            raise HTTPException(status_code=404, detail=f"run {run_id} not found")
        return summary
    path = _run_dir(run_id) / "summary.json"
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    return _read_json_object(path)


@app.get("/api/run/{run_id}/trace")
def get_trace(run_id: str) -> list[dict[str, Any]]:
    trace = _run_dir(run_id) / "trace.jsonl"
    if not trace.exists():
        raise HTTPException(status_code=404, detail=f"trace for {run_id} not found")
    events: list[dict[str, Any]] = []
    for line in trace.read_text(encoding="utf-8").splitlines():
        if line.strip():
            events.append(json.loads(line))
    return events


@app.get("/api/run/{run_id}/patch")
def get_patch(run_id: str) -> dict[str, Any]:
    patch = _run_dir(run_id) / "final.patch"
    return {"patch": patch.read_text(encoding="utf-8") if patch.exists() else ""}


@app.get("/api/calibration")
def get_calibration(suite: str | None = None) -> dict[str, Any]:
    """Empirical difficulty calibration from recorded runs."""
    rows = _rows()
    if suite in SUITE_ALIASES:
        allowed = set(load_suite(suite).task_ids)
        rows = [r for r in rows if r.get("task_id") in allowed]
    elif suite:
        rows = [r for r in rows if r.get("suite") == suite]
    return calibrate_tasks(rows, TASKS_ROOT)


@app.get("/api/effort-sensitivity")
def get_effort_sensitivity(suite: str | None = None) -> dict[str, Any]:
    """Effort-sensitivity strata from effort-tagged runs."""
    rows = _rows()
    if suite in SUITE_ALIASES:
        allowed = set(load_suite(suite).task_ids)
        rows = [r for r in rows if r.get("task_id") in allowed]
    elif suite:
        rows = [r for r in rows if r.get("suite") == suite]
    return build_effort_sensitivity(rows, TASKS_ROOT)


@app.get("/api/tasks")
def get_tasks() -> list[dict[str, Any]]:
    """List the benchmark tasks with their metadata and calibration data."""
    rows = _rows()
    cal = calibrate_tasks(rows, TASKS_ROOT)
    cal_by_id: dict[str, dict[str, Any]] = {e["task_id"]: e for e in cal["tasks"]}
    out: list[dict[str, Any]] = []
    for task_id in list_task_ids(TASKS_ROOT):
        try:
            task = load_task(task_id, TASKS_ROOT)
        except (OSError, ValueError):
            continue
        m = task.metadata
        entry = cal_by_id.get(task_id, {})
        out.append(
            {
                "id": task_id,
                "category": m.get("category"),
                "languages": m.get("languages", []),
                "difficulty": m.get("difficulty"),
                "task_complexity": m.get("task_complexity"),
                "source": m.get("source"),
                "empirical_difficulty": entry.get("empirical_difficulty"),
                "solve_rate": entry.get("solve_rate"),
                "calibration_status": entry.get("status"),
            }
        )
    return out


@app.get("/api/task/{task_id}")
def get_task(task_id: str) -> dict[str, Any]:
    """Task metadata + issue + every run recorded for it."""
    if task_id not in list_task_ids(TASKS_ROOT):
        raise HTTPException(status_code=404, detail=f"task {task_id} not found")
    task = load_task(task_id, TASKS_ROOT)
    runs = [r for r in _rows() if r.get("task_id") == task_id]
    runs.sort(key=lambda r: r.get("total") or 0, reverse=True)
    return {"id": task_id, "metadata": task.metadata, "issue": task.issue, "runs": runs}


def _run_dir(run_id: str) -> Path:
    """Resolve a run directory, refusing ids that escape RUNS_DIR."""
    base = RUNS_DIR.resolve()
    target = (base / run_id).resolve()
    if not target.is_relative_to(base):
        raise HTTPException(status_code=400, detail="invalid run id")
    return target


def _read_json_object(path: Path) -> dict[str, Any]:
    data: Any = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise HTTPException(status_code=500, detail=f"{path.name} must contain a JSON object")
    return cast(dict[str, Any], data)


# Serve raw run artifacts (replay.html, final.patch, trace.jsonl) at /runs/<id>/...
if RUNS_DIR.exists():
    app.mount("/runs", StaticFiles(directory=str(RUNS_DIR)), name="runs")

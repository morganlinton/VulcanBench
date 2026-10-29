"""Read-only runner routes: what is running or finished, logs, first-attempt checks.

Served only when ``VULCANBENCH_RUNNER=1`` and only to loopback clients. The
log endpoint serves an allowlist of files (see ``discovery``) through the
redactor; nothing under ``agent/sessions`` or ``agent/setup`` is reachable.
"""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends, HTTPException, Query, Request

from backend.runner import docker_ps
from backend.runner.config import load_config
from backend.runner.discovery import Discovery, tail_text
from backend.runner.redact import redact

LOOPBACK_HOSTS = frozenset({"127.0.0.1", "::1", "::ffff:127.0.0.1"})


def require_runner(request: Request) -> None:
    """Refuse unless the runner is enabled and the client is on this machine."""
    if not load_config().enabled:
        raise HTTPException(status_code=404, detail="runner disabled (set VULCANBENCH_RUNNER=1)")
    host = request.client.host if request.client else None
    if host not in LOOPBACK_HOSTS:
        raise HTTPException(status_code=403, detail="runner routes are served to localhost only")


router = APIRouter(prefix="/api/runner", tags=["runner"], dependencies=[Depends(require_runner)])


@router.get("/status")
def runner_status() -> dict[str, Any]:
    cfg = load_config()
    return {
        "config": cfg.to_dict(),
        "docker_available": docker_ps.docker_available(),
        "gate_jobs_dir_exists": cfg.gate_jobs_dir.is_dir(),
        "run_roots_present": [str(p) for p in cfg.run_roots if p.is_dir()],
        "supervisor": "not available in this version",
    }


@router.get("/runs")
def list_runs() -> list[dict[str, Any]]:
    return [r.summary_dict() for r in Discovery(load_config()).all_runs()]


@router.get("/runs/{run_id:path}/log")
def run_log(
    run_id: str,
    source: str = Query(..., min_length=1, max_length=200),
    trial: str | None = Query(default=None, max_length=200),
    lines: int = Query(default=200, ge=1, le=2000),
) -> dict[str, Any]:
    disc = Discovery(load_config())
    run = disc.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    path = disc.resolve_log(run, source, trial)
    if path is None:
        raise HTTPException(status_code=404, detail="no such log for this run")
    text, truncated = tail_text(path, max_lines=lines)
    try:
        size = path.stat().st_size
    except OSError:
        size = 0
    return {
        "run_id": run.id,
        "trial": trial,
        "source": source,
        "size": size,
        "truncated": truncated,
        "redacted": True,
        "text": redact(text),
    }


@router.get("/runs/{run_id:path}/check")
def run_check(run_id: str) -> dict[str, Any]:
    run = Discovery(load_config()).get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    d = run.to_dict()
    return {
        "run_id": run.id,
        "first_check": d["first_check"],
        "trials": [
            {"id": t["id"], "status": t["status"], "first_check": t["first_check"]}
            for t in d["trials"]
        ],
    }


@router.get("/runs/{run_id:path}")
def get_run_view(run_id: str) -> dict[str, Any]:
    run = Discovery(load_config()).get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    return run.to_dict()

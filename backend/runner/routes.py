"""Runner routes: what is running or finished, logs, first-attempt checks, launch plans, the queue.

Served only when ``VULCANBENCH_RUNNER=1`` and only to loopback clients. The
log endpoint serves an allowlist of files (see ``discovery``) through the
redactor; nothing under ``agent/sessions`` or ``agent/setup`` is reachable.
Nothing that spends model quota starts from here except through
``POST /plans/{id}/confirm`` with the plan's hash, and even that only queues
the job; the supervisor starts it.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Literal

from fastapi import APIRouter, Depends, HTTPException, Query, Request, Response
from pydantic import BaseModel, Field

from backend.runner import docker_ps, launch, procs
from backend.runner.config import RunnerConfig, load_config
from backend.runner.discovery import Discovery, job_summary, tail_text
from backend.runner.redact import redact
from backend.runner.state import PlanError, State, parse
from backend.runner.supervisor import SERVICE

LOOPBACK_HOSTS = frozenset({"127.0.0.1", "::1", "::ffff:127.0.0.1"})
USAGE_CHECK_MAX_AGE_S = 600


def require_runner(request: Request) -> None:
    """Refuse unless the runner is enabled and the client is on this machine."""
    if not load_config().enabled:
        raise HTTPException(status_code=404, detail="runner disabled (set VULCANBENCH_RUNNER=1)")
    host = request.client.host if request.client else None
    if host not in LOOPBACK_HOSTS:
        raise HTTPException(status_code=403, detail="runner routes are served to localhost only")


router = APIRouter(prefix="/api/runner", tags=["runner"], dependencies=[Depends(require_runner)])


def _state(cfg: RunnerConfig) -> State:
    return State(cfg.home / "runner.db")


def _discovery(cfg: RunnerConfig, state: State) -> Discovery:
    return Discovery(cfg, jobs=state.list_jobs(limit=200))


# ------------------------------------------------------------------ reads --


@router.get("/status")
def runner_status() -> dict[str, Any]:
    cfg = load_config()
    return {
        "config": cfg.to_dict(),
        "docker_available": docker_ps.docker_available(),
        "gate_jobs_dir_exists": cfg.gate_jobs_dir.is_dir(),
        "run_roots_present": [str(p) for p in cfg.run_roots if p.is_dir()],
        "supervisor": SERVICE.describe(),
    }


@router.get("/runs")
def list_runs() -> list[dict[str, Any]]:
    cfg = load_config()
    return [r.summary_dict() for r in _discovery(cfg, _state(cfg)).all_runs()]


@router.get("/runs/{run_id:path}/log")
def run_log(
    run_id: str,
    source: str = Query(..., min_length=1, max_length=200),
    trial: str | None = Query(default=None, max_length=200),
    lines: int = Query(default=200, ge=1, le=2000),
) -> dict[str, Any]:
    cfg = load_config()
    disc = _discovery(cfg, _state(cfg))
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
    cfg = load_config()
    run = _discovery(cfg, _state(cfg)).get(run_id)
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
    cfg = load_config()
    run = _discovery(cfg, _state(cfg)).get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    return run.to_dict()


# ---------------------------------------------------------------- launch --


@router.get("/options")
def launch_options(tag: str | None = Query(default=None, max_length=200)) -> dict[str, Any]:
    return launch.options(load_config(), tag)


class PlanRequest(BaseModel):
    kind: Literal["gate"] = "gate"
    ref: str = Field(min_length=1, max_length=40)
    task: str = Field(min_length=1, max_length=120)
    tag: str = Field(min_length=1, max_length=200)
    attempts: int = Field(default=3, ge=1, le=3)
    concurrency: int | None = Field(default=None, ge=1, le=2)
    suffix: str = Field(default="", max_length=40)


class ConfirmRequest(BaseModel):
    hash: str = Field(min_length=8, max_length=64)


class StopRequest(BaseModel):
    reason: str = Field(default="stopped from the GUI", max_length=200)


def _plan_dict(state: State, plan_id: str) -> dict[str, Any]:
    plan = state.get_plan(plan_id)
    if plan is None:
        raise HTTPException(status_code=404, detail="plan not found")
    d = plan.to_dict()
    d["live_reason"] = state.plan_is_live(plan)
    d["usage_check_ok"] = _usage_check_ok(state, plan.spec, plan.usage_check, plan.usage_checked_at)
    return d


def _usage_check_ok(
    state: State, spec: dict[str, Any], check: dict[str, Any] | None, checked_at: str | None
) -> bool:
    if not spec.get("usage_check_required"):
        return True
    if not check or not checked_at:
        return False
    at = parse(checked_at)
    if at is None or (state.now() - at).total_seconds() > USAGE_CHECK_MAX_AGE_S:
        return False
    return bool(check.get("would_start")) and check.get("status") in {
        "allowed",
        "unknown",
        "unavailable",
    }


@router.post("/plans", status_code=201)
def create_plan(body: PlanRequest) -> dict[str, Any]:
    cfg = load_config()
    form = launch.GateForm(
        ref=body.ref,
        task=body.task,
        tag=body.tag,
        attempts=body.attempts,
        concurrency=body.concurrency,
        suffix=body.suffix,
    )
    try:
        spec = launch.build_gate_plan(cfg, form)
    except launch.LaunchError as exc:
        raise HTTPException(status_code=exc.status, detail=exc.detail) from exc
    state = _state(cfg)
    plan = state.create_plan("gate", spec, spec["hash"])
    return _plan_dict(state, plan.id)


@router.get("/plans/{plan_id}")
def get_plan(plan_id: str) -> dict[str, Any]:
    return _plan_dict(_state(load_config()), plan_id)


@router.post("/plans/{plan_id}/usage")
def check_usage(plan_id: str) -> dict[str, Any]:
    """Run the gate script's usage pre-flight (one short model turn). Explicit click only."""
    cfg = load_config()
    state = _state(cfg)
    plan = state.get_plan(plan_id)
    if plan is None:
        raise HTTPException(status_code=404, detail="plan not found")
    reason = state.plan_is_live(plan)
    if reason:
        raise HTTPException(status_code=409, detail=reason)
    result = launch.run_usage_preflight(cfg, plan.spec)
    state.set_usage_check(plan_id, result)
    return _plan_dict(state, plan_id)


@router.post("/plans/{plan_id}/confirm", status_code=201)
def confirm_plan(plan_id: str, body: ConfirmRequest) -> dict[str, Any]:
    cfg = load_config()
    state = _state(cfg)
    plan = state.get_plan(plan_id)
    if plan is None:
        raise HTTPException(status_code=404, detail="plan not found")
    if plan.hash != body.hash or plan.spec.get("hash") != body.hash:
        raise HTTPException(status_code=409, detail="plan hash does not match what was confirmed")
    reason = state.plan_is_live(plan)
    if reason:
        raise HTTPException(status_code=409, detail=reason)
    if not _usage_check_ok(state, plan.spec, plan.usage_check, plan.usage_checked_at):
        raise HTTPException(
            status_code=409,
            detail="run the usage check first (it must be at most 10 minutes old and say the run would start)",
        )
    if plan.spec.get("needs_prep"):
        problems = launch.validate_worktree(cfg, str(plan.spec.get("tag")))
        if launch.worktree_dir(cfg, str(plan.spec.get("tag"))).is_dir():
            problems = launch.validate_before_launch(cfg, plan.spec)
        elif Path(str(plan.spec.get("job_dir"))).exists():
            problems.append("job directory already exists")
    else:
        problems = launch.validate_before_launch(cfg, plan.spec)
    if problems:
        raise HTTPException(status_code=409, detail="; ".join(problems))
    try:
        job = state.confirm_plan(plan_id, body.hash)
    except PlanError as exc:
        raise HTTPException(status_code=exc.status, detail=exc.detail) from exc
    return job_summary(job)


# ----------------------------------------------------------------- queue --


@router.get("/queue")
def queue() -> dict[str, Any]:
    cfg = load_config()
    state = _state(cfg)
    jobs = state.list_jobs(limit=50)
    table = procs.list_processes()
    outside = [
        {"pid": p.pid, "command": redact(p.command[:300])}
        for p in table
        if ("harbor run" in p.command or "vulcanbench run " in p.command)
        and not p.zombie
        and p.pid not in {j.pid for j in jobs if j.pid}
    ]
    return {
        "jobs": [job_summary(j) for j in jobs],
        "supervisor": SERVICE.describe(),
        "outside": outside,
    }


@router.get("/jobs/{job_id}")
def get_job(job_id: int, lines: int = Query(default=200, ge=1, le=2000)) -> dict[str, Any]:
    cfg = load_config()
    job = _state(cfg).get_job(job_id)
    if job is None:
        raise HTTPException(status_code=404, detail="job not found")
    d = job.to_dict()
    console = ""
    if job.console_log and Path(job.console_log).is_file():
        console, _ = tail_text(Path(job.console_log), max_lines=lines)
    d["console"] = redact(console)
    return d


@router.post("/jobs/{job_id}/stop")
def stop_job(job_id: int, body: StopRequest | None = None) -> dict[str, Any]:
    cfg = load_config()
    state = _state(cfg)
    job = state.request_stop(job_id, (body or StopRequest()).reason)
    if job is None:
        raise HTTPException(status_code=409, detail="job is not queued or running")
    return job_summary(job)


@router.delete("/jobs/{job_id}", status_code=204)
def delete_queued_job(job_id: int) -> Response:
    cfg = load_config()
    if not _state(cfg).delete_queued(job_id):
        raise HTTPException(status_code=409, detail="only queued jobs can be removed")
    return Response(status_code=204)

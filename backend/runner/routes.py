"""Runner routes: what is running or finished, logs, first-attempt checks, launch plans, the queue.

Served only when ``VULCANBENCH_RUNNER=1`` and only to loopback clients. The
log endpoint serves an allowlist of files (see ``discovery``) through the
redactor; nothing under ``agent/sessions`` or ``agent/setup`` is reachable.
Nothing that spends model quota starts from here except through
``POST /plans/{id}/confirm`` with the plan's hash, and even that only queues
the job; the supervisor starts it.
"""

from __future__ import annotations

from datetime import UTC, datetime
from pathlib import Path
from typing import Annotated, Any, Literal

from fastapi import APIRouter, Depends, HTTPException, Query, Request, Response
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field

from backend.runner import docker_ps, launch, maintenance, procs, stream, sweep
from backend.runner.config import RunnerConfig, load_config
from backend.runner.discovery import HARNESS_PREFIX, Discovery, job_summary, tail_text
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


@router.get("/runs/{run_id:path}/log/stream")
def run_log_stream(
    run_id: str,
    source: str = Query(..., min_length=1, max_length=200),
    trial: str | None = Query(default=None, max_length=200),
    lines: int = Query(default=300, ge=1, le=2000),
    max_s: float = Query(default=600.0, ge=0.0, le=3600.0),
) -> StreamingResponse:
    """The same tail as ``/log``, then only appended bytes, as server-sent events."""
    cfg = load_config()
    disc = _discovery(cfg, _state(cfg))
    run = disc.get(run_id)
    if run is None:
        raise HTTPException(status_code=404, detail=f"run {run_id} not found")
    path = disc.resolve_log(run, source, trial)
    if path is None:
        raise HTTPException(status_code=404, detail="no such log for this run")
    return StreamingResponse(
        stream.log_events(path, lines=lines, max_s=max_s),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


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
    cfg = load_config()
    opts = launch.options(cfg, tag)
    opts["kinds"] = ["sweep", "gate", "prepare"]
    opts.update(sweep.suite_options(cfg, opts.get("tag")))
    return opts


class GatePlanRequest(BaseModel):
    kind: Literal["gate"] = "gate"
    ref: str = Field(min_length=1, max_length=40)
    task: str = Field(min_length=1, max_length=120)
    tag: str = Field(min_length=1, max_length=200)
    attempts: int = Field(default=3, ge=1, le=3)
    concurrency: int | None = Field(default=None, ge=1, le=2)
    suffix: str = Field(default="", max_length=40)


class SweepPlanRequest(BaseModel):
    """Benchmark a model: one suite, one harness, one job per effort level."""

    kind: Literal["sweep"]
    model: str = Field(min_length=1, max_length=120)
    harness: str = Field(min_length=1, max_length=40)
    suite: str = Field(min_length=1, max_length=120)
    tag: str = Field(min_length=1, max_length=200)
    efforts: list[str] = Field(min_length=1, max_length=7)
    billing: str = Field(default="auto", max_length=20)
    repeat: int = Field(default=1, ge=1, le=sweep.MAX_REPEAT)
    only_missing: bool = True
    sandbox: Literal["local", "docker"] | None = None
    quota_continue: int = Field(default=0, ge=0, le=sweep.QUOTA_CONTINUE_MAX)
    label: str = Field(default="", max_length=40)
    cli_bin_dir: str = Field(default="", max_length=300)


class PreparePlanRequest(BaseModel):
    """A quota-free queue step: create the run worktree for a bench tag."""

    kind: Literal["prepare"]
    tag: str = Field(min_length=1, max_length=200)


PlanRequest = Annotated[
    GatePlanRequest | SweepPlanRequest | PreparePlanRequest, Field(discriminator="kind")
]


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
    try:
        if isinstance(body, SweepPlanRequest):
            form = sweep.SuiteForm(
                model=body.model.strip(),
                harness=body.harness,
                suite=body.suite,
                tag=body.tag,
                efforts=tuple(body.efforts),
                billing=body.billing,
                repeat=body.repeat,
                only_missing=body.only_missing,
                sandbox=body.sandbox,
                quota_continue=body.quota_continue,
                label=body.label.strip(),
                cli_bin_dir=body.cli_bin_dir.strip(),
            )
            spec = sweep.build_suite_plan(cfg, form, today=datetime.now(UTC).strftime("%Y-%m-%d"))
            spec["hash"] = launch.plan_hash(spec)
        elif isinstance(body, PreparePlanRequest):
            spec = launch.build_prepare_plan(cfg, body.tag)
        else:
            gate = launch.GateForm(
                ref=body.ref,
                task=body.task,
                tag=body.tag,
                attempts=body.attempts,
                concurrency=body.concurrency,
                suffix=body.suffix,
            )
            spec = launch.build_gate_plan(cfg, gate)
    except launch.LaunchError as exc:
        raise HTTPException(status_code=exc.status, detail=exc.detail) from exc
    state = _state(cfg)
    plan = state.create_plan(str(spec["kind"]), spec, spec["hash"])
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
    problems = _confirm_problems(cfg, plan.spec)
    if problems:
        raise HTTPException(status_code=409, detail="; ".join(problems))
    try:
        jobs = state.confirm_plan(plan_id, body.hash)
    except PlanError as exc:
        raise HTTPException(status_code=exc.status, detail=exc.detail) from exc
    out = job_summary(jobs[0])
    out["jobs"] = [job_summary(j) for j in jobs]
    return out


def _confirm_problems(cfg: RunnerConfig, spec: dict[str, Any]) -> list[str]:
    kind = spec.get("kind")
    if kind == "sweep":
        doctor = spec.get("doctor") or {}
        problems: list[str] = []
        if doctor.get("status") == "not_ready":
            problems.append(f"harness not ready: {doctor.get('detail') or 'see the plan'}")
        for entry in spec.get("jobs") or []:
            problems.extend(sweep.validate_suite_launch(cfg, entry))
            if Path(str(entry.get("out_dir"))).exists() and not entry.get("only_missing"):
                problems.append(f"{entry.get('out_dir')} exists and --only-missing is off")
        return sorted(set(problems))
    if kind == "prepare":
        if launch.worktree_dir(cfg, str(spec.get("tag"))).is_dir():
            return ["the worktree already exists"]
        return launch.validate_before_launch(cfg, spec)
    if spec.get("needs_prep"):
        problems = launch.validate_worktree(cfg, str(spec.get("tag")))
        if launch.worktree_dir(cfg, str(spec.get("tag"))).is_dir():
            problems = launch.validate_before_launch(cfg, spec)
        elif Path(str(spec.get("job_dir"))).exists():
            problems.append("job directory already exists")
        return problems
    return launch.validate_before_launch(cfg, spec)


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


@router.get("/sweeps")
def sweeps(limit: int = Query(default=20, ge=1, le=100)) -> dict[str, Any]:
    """Model sweeps the GUI launched, newest first, with per-level progress from disk."""
    cfg = load_config()
    state = _state(cfg)
    now = datetime.now(UTC)
    groups: dict[str, dict[str, Any]] = {}
    for job in state.list_jobs(limit=500):
        if job.kind != "suite":
            continue
        sid = str(job.spec.get("sweep_id") or job.name)
        group = groups.setdefault(
            sid,
            {
                "sweep_id": sid,
                "name": job.spec.get("sweep_name") or job.name,
                "model": job.spec.get("model"),
                "harness": job.spec.get("harness"),
                "suite": job.spec.get("suite"),
                "suite_name": job.spec.get("suite_name"),
                "tag": job.spec.get("tag"),
                "tag_commit": job.spec.get("tag_commit"),
                "created_at": job.created_at,
                "levels": [],
            },
        )
        prog = sweep.progress(job.spec, now)
        current = prog.get("current")
        if current and current.get("path"):
            try:
                rel = Path(str(current["path"])).relative_to(cfg.harness_runs_dir).as_posix()
                current["run_id_href"] = f"{HARNESS_PREFIX}0:{rel}"
            except ValueError:
                current["run_id_href"] = None
        group["levels"].append({"job": job_summary(job), "progress": prog})
        group["created_at"] = min(group["created_at"], job.created_at)
    out = sorted(groups.values(), key=lambda g: g["created_at"], reverse=True)[:limit]
    for g in out:
        g["levels"].sort(key=lambda lv: lv["job"]["id"])
        levels = g["levels"]
        g["totals"] = {
            "runs_done": sum(lv["progress"]["runs_done"] for lv in levels),
            "total_runs": sum(lv["progress"]["total_runs"] for lv in levels),
            "passed": sum(lv["progress"]["passed"] for lv in levels),
            "cost_usd": (
                round(sum(lv["progress"]["cost_usd"] or 0 for lv in levels), 4)
                if any(lv["progress"]["cost_usd"] is not None for lv in levels)
                else None
            ),
            "active": any(
                lv["job"]["status"] in {"queued", "waiting", "preparing", "running", "stopping"}
                for lv in levels
            ),
        }
    return {"sweeps": out}


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


# ----------------------------------------------------------- maintenance --


class RemoveRequest(BaseModel):
    ids: list[str] = Field(min_length=1, max_length=200)


class TagRequest(BaseModel):
    label: str = Field(min_length=1, max_length=60)
    push: bool = False


@router.get("/maintenance/orphans")
def orphans() -> dict[str, Any]:
    cfg = load_config()
    return maintenance.orphan_containers(cfg, _state(cfg))


@router.post("/maintenance/orphans/remove")
def remove_orphans(body: RemoveRequest) -> dict[str, Any]:
    """Remove only ids that are orphans at this moment; anything else is refused by id."""
    cfg = load_config()
    docker_ps.reset_cache()
    current = maintenance.orphan_containers(cfg, _state(cfg))["containers"]
    result = maintenance.remove_containers(body.ids, current)
    docker_ps.reset_cache()
    return result


@router.get("/maintenance/tag")
def tag_preflight() -> dict[str, Any]:
    return maintenance.tag_preflight()


@router.post("/maintenance/tag", status_code=201)
def cut_tag(body: TagRequest) -> dict[str, Any]:
    """``scripts/cut_bench_tag.sh <label> [--push]`` in the backend's checkout; its guards decide."""
    result = maintenance.cut_bench_tag(body.label.strip(), push=body.push)
    if not result["ok"]:
        raise HTTPException(status_code=409, detail=result)
    return result


@router.delete("/jobs/{job_id}", status_code=204)
def delete_queued_job(job_id: int) -> Response:
    cfg = load_config()
    if not _state(cfg).delete_queued(job_id):
        raise HTTPException(status_code=409, detail="only queued or waiting jobs can be removed")
    return Response(status_code=204)

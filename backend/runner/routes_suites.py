"""Suites routes: build your own eval suite from merged pull requests (``backend/runner/suites.py``).

Same guard as the other runner routes (``VULCANBENCH_RUNNER=1``, loopback
clients, pages served from localhost). Nothing here spends model quota:
listing pull requests and scaffolding are network-bound but quota-free and
run synchronously; authoring and validation are queued jobs that go through
``POST /api/runner/plans`` and its confirm click like every other run.

``POST /suites/unlock`` takes the builder password. It reads the request
body itself rather than through a pydantic model, because FastAPI's 422
response echoes the rejected input back; nothing here logs a request body,
and the password reaches the engine only on the child's stdin.
"""

from __future__ import annotations

import json
from typing import Any

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from pydantic import BaseModel, Field
from starlette.concurrency import run_in_threadpool

from backend.runner import suites
from backend.runner.config import RunnerConfig, load_config
from backend.runner.routes import require_runner
from backend.runner.state import Job, State

router = APIRouter(
    prefix="/api/runner/suites", tags=["runner-suites"], dependencies=[Depends(require_runner)]
)

MAX_UNLOCK_BODY = 4096


def _jobs(cfg: RunnerConfig) -> list[Job]:
    return [
        j
        for j in State(cfg.home / "runner.db").list_jobs(limit=500)
        if j.kind in suites.BUILDER_KINDS
    ]


def _http(exc: suites.SuitesError) -> HTTPException:
    return HTTPException(status_code=exc.status, detail=exc.detail)


class NewSuiteRequest(BaseModel):
    id: str = Field(min_length=3, max_length=64)
    display_name: str = Field(min_length=1, max_length=120)
    repo: str = Field(min_length=3, max_length=200)
    tag: str | None = Field(default=None, max_length=200)


class ScaffoldRequest(BaseModel):
    pr: int = Field(ge=1, le=10_000_000)
    tag: str | None = Field(default=None, max_length=200)
    # Terms for repo/NOTICE when the repository has no LICENSE file (a private repo, say).
    license_note: str = Field(default="", max_length=suites.MAX_LICENSE_NOTE_CHARS)


class TagOnly(BaseModel):
    tag: str | None = Field(default=None, max_length=200)


class DropRequest(BaseModel):
    reason: str = Field(default="", max_length=300)
    tag: str | None = Field(default=None, max_length=200)


# --------------------------------------------------------------- access --


@router.get("/access")
def get_access(tag: str | None = Query(default=None, max_length=200)) -> dict[str, Any]:
    """Whether the builder is unlocked on this Mac, from ``suite access --json``."""
    try:
        return suites.access_status(load_config(), tag)
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.post("/unlock")
async def post_unlock(request: Request) -> dict[str, Any]:
    """Pipe the password to ``vulcanbench suite unlock --password-stdin``. Never logged."""
    raw = await request.body()
    if len(raw) > MAX_UNLOCK_BODY:
        raise HTTPException(status_code=413, detail={"code": "bad_request", "message": "too large"})
    try:
        body = json.loads(raw.decode("utf-8")) if raw else None
    except (UnicodeDecodeError, json.JSONDecodeError):
        body = None
    del raw
    try:
        password = suites.check_password_shape(
            body.get("password") if isinstance(body, dict) else None
        )
        tag = body.get("tag") if isinstance(body, dict) else None
        if tag is not None and (not isinstance(tag, str) or len(tag) > 200):
            raise suites.SuitesError(422, {"code": "bad_request", "message": "bad tag"})
        del body
        return await run_in_threadpool(suites.unlock, load_config(), password, tag=tag)
    except suites.SuitesError as exc:
        raise _http(exc) from None


@router.post("/lock")
def post_lock(body: TagOnly | None = None) -> dict[str, Any]:
    try:
        return suites.lock(load_config(), tag=(body or TagOnly()).tag)
    except suites.SuitesError as exc:
        raise _http(exc) from exc


# ---------------------------------------------------------------- suites --


@router.get("")
def list_suites(tag: str | None = Query(default=None, max_length=200)) -> dict[str, Any]:
    cfg = load_config()
    return {
        "suites_home": str(cfg.suites_home),
        "tooling": suites.tooling_worktree(cfg, tag),
        "suites": suites.list_suites(cfg, _jobs(cfg)),
    }


@router.post("", status_code=201)
def create_suite(body: NewSuiteRequest) -> dict[str, Any]:
    try:
        return suites.create_suite(
            load_config(),
            suite_id=body.id.strip(),
            display_name=body.display_name,
            repo=body.repo.strip(),
            tag=body.tag,
        )
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.get("/{suite_id}")
def get_suite(
    suite_id: str, tag: str | None = Query(default=None, max_length=200)
) -> dict[str, Any]:
    cfg = load_config()
    try:
        out = suites.suite_detail(cfg, suite_id, _jobs(cfg))
    except suites.SuitesError as exc:
        raise _http(exc) from exc
    out["tooling"] = suites.tooling_worktree(cfg, tag)
    return out


@router.get("/{suite_id}/prs")
def get_prs(
    suite_id: str,
    since: str | None = Query(default=None, max_length=10),
    limit: int = Query(default=50, ge=1, le=200),
    refresh: bool = False,
    tag: str | None = Query(default=None, max_length=200),
) -> dict[str, Any]:
    try:
        return suites.list_prs(
            load_config(), suite_id, since=since or None, limit=limit, refresh=refresh, tag=tag
        )
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.post("/{suite_id}/tasks", status_code=201)
def post_scaffold(suite_id: str, body: ScaffoldRequest) -> dict[str, Any]:
    try:
        return suites.scaffold_task(
            load_config(), suite_id, pr=body.pr, tag=body.tag, license_note=body.license_note
        )
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.get("/{suite_id}/tasks/{task}")
def get_task(suite_id: str, task: str) -> dict[str, Any]:
    cfg = load_config()
    try:
        return suites.task_detail(cfg, suite_id, task, _jobs(cfg))
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.post("/{suite_id}/tasks/{task}/accept")
def post_accept(suite_id: str, task: str, body: TagOnly | None = None) -> dict[str, Any]:
    cfg = load_config()
    try:
        return suites.accept_task(cfg, suite_id, task, _jobs(cfg), tag=(body or TagOnly()).tag)
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.post("/{suite_id}/tasks/{task}/drop")
def post_drop(suite_id: str, task: str, body: DropRequest | None = None) -> dict[str, Any]:
    cfg = load_config()
    req = body or DropRequest()
    try:
        return suites.drop_task(cfg, suite_id, task, _jobs(cfg), reason=req.reason, tag=req.tag)
    except suites.SuitesError as exc:
        raise _http(exc) from exc


@router.post("/{suite_id}/freeze", status_code=201)
def post_freeze(suite_id: str, body: TagOnly | None = None) -> dict[str, Any]:
    cfg = load_config()
    try:
        return suites.freeze_suite(cfg, suite_id, _jobs(cfg), tag=(body or TagOnly()).tag)
    except suites.SuitesError as exc:
        raise _http(exc) from exc

"""Suite builder jobs: author and validate plans, confirm, and how the supervisor ends them.

Plans go through ``POST /api/runner/plans`` like benchmark sweeps; the
supervisor then runs the fake ``vulcanbench suite task author`` or ``suite
validate`` (``tests/runner_fixtures.py``) as a real child process.
"""

from __future__ import annotations

import os
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from fastapi.testclient import TestClient

from backend import app as appmod
from backend.runner import docker_ps, procs, suites
from backend.runner.state import State
from backend.runner.supervisor import AGENT_KILL_AFTER_S, Supervisor
from tests.runner_fixtures import (
    BUILDER_TAG,
    BuilderEnv,
    builder_env,
    fake_builder_task,
    set_fake_suite,
)

PLANS = "/api/runner/plans"


@pytest.fixture
def env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> BuilderEnv:
    e = builder_env(tmp_path, monkeypatch)
    set_fake_suite(e.wt, "unlocked")
    monkeypatch.setattr(docker_ps, "docker_available", lambda: True)
    fake_builder_task(e.cfg, "acme-api", "t-one", pr=1)
    fake_builder_task(e.cfg, "acme-api", "t-two", pr=2, author="ok")
    return e


def _client() -> TestClient:
    return TestClient(appmod.app, client=("127.0.0.1", 50000))


def _author_body(**over: Any) -> dict[str, Any]:
    body: dict[str, Any] = {
        "kind": "author",
        "suite": "acme-api",
        "tasks": ["t-one", "t-two"],
        "harness": "codex",
        "model": "gpt-6.1-terra",
        "billing": "subscription",
        "effort": "medium",
    }
    body.update(over)
    return body


# --------------------------------------------------------------- plans --


def test_author_plan_has_one_job_per_task_and_the_exact_commands(env: BuilderEnv) -> None:
    r = _client().post(PLANS, json=_author_body())
    assert r.status_code == 201, r.text
    plan = r.json()
    spec = plan["spec"]
    assert plan["kind"] == "author" and len(spec["jobs"]) == 2
    cli = str(env.wt / ".venv" / "bin" / "vulcanbench")
    assert spec["jobs"][0]["argv"] == [
        cli, "suite", "task", "author", "acme-api", "--task", "t-one",
        "--harness", "codex", "--model", "gpt-6.1-terra", "--billing", "subscription",
        "--effort", "medium", "--timeout", "3600", "--max-turns", "200",
        "--suites-home", str(env.cfg.suites_home), "--json",
    ]  # fmt: skip
    assert spec["jobs"][0]["env"] == {} and spec["jobs"][0]["cwd"] == str(env.wt)
    assert spec["tag"] == BUILDER_TAG
    est = spec["estimate"]
    assert est["sessions"] == 2 and est["max_wall_s"] == 2 * 3600
    assert "nothing retries" in est["quota"]
    assert spec["doctor"]["status"] == "ready"
    assert any("t-two was authored before" in w for w in spec["warnings"])
    assert any("network off" in w for w in spec["warnings"])


@pytest.mark.parametrize(
    ("over", "status"),
    [
        ({"harness": "vulcan"}, 422),
        ({"effort": "ultra"}, 422),
        ({"model": "openai:gpt-6"}, 422),
        ({"billing": "api"}, 422),
        ({"tasks": ["t-one", "t-one"]}, 422),
        ({"tasks": ["no-such"]}, 404),
        ({"suite": "nope-suite"}, 404),
    ],
)
def test_author_plan_refusals(env: BuilderEnv, over: dict[str, Any], status: int) -> None:
    assert _client().post(PLANS, json=_author_body(**over)).status_code == status


def test_author_plan_refused_when_locked_frozen_or_dropped(env: BuilderEnv) -> None:
    c = _client()
    (env.wt / ".fake-suite-unlocked").unlink()
    locked = c.post(PLANS, json=_author_body())
    assert locked.status_code == 403 and locked.json()["detail"]["code"] == "builder_locked"
    set_fake_suite(env.wt, "unlocked")
    d = env.cfg.suites_home / "acme-api"
    (d / "t-one" / "build" / "dropped.json").write_text("{}")
    assert c.post(PLANS, json=_author_body()).status_code == 409
    (d / "suite.lock.json").write_text("{}")
    assert c.post(PLANS, json=_author_body(tasks=["t-two"])).status_code == 409


def test_confirm_queues_author_jobs_and_a_not_ready_doctor_blocks_it(env: BuilderEnv) -> None:
    c = _client()
    (env.wt / ".fake-doctor-not-ready").write_text("1")
    plan = c.post(PLANS, json=_author_body()).json()
    assert any("not ready" in w for w in plan["spec"]["warnings"])
    refused = c.post(f"{PLANS}/{plan['id']}/confirm", json={"hash": plan["hash"]})
    assert refused.status_code == 409 and "harness not ready" in refused.text
    (env.wt / ".fake-doctor-not-ready").unlink()
    plan = c.post(PLANS, json=_author_body()).json()
    ok = c.post(f"{PLANS}/{plan['id']}/confirm", json={"hash": plan["hash"]})
    assert ok.status_code == 201
    jobs = ok.json()["jobs"]
    assert [j["kind"] for j in jobs] == ["author", "author"]
    assert [j["spec"]["task"] for j in jobs] == ["t-one", "t-two"]
    detail = c.get("/api/runner/suites/acme-api").json()
    assert {t["id"]: t["stage"] for t in detail["tasks"]} == {
        "t-one": "authoring",
        "t-two": "authoring",
    }
    again = c.post(PLANS, json=_author_body(tasks=["t-one"]))
    assert again.status_code == 409  # an active job per task


def test_validate_plan_needs_an_authored_task_and_docker(
    env: BuilderEnv, monkeypatch: pytest.MonkeyPatch
) -> None:
    c = _client()
    body = {"kind": "validate", "suite": "acme-api", "tasks": ["t-one"]}
    early = c.post(PLANS, json=body)
    assert early.status_code == 409 and "author it first" in early.text
    body["tasks"] = ["t-two"]
    plan = c.post(PLANS, json=body)
    assert plan.status_code == 201
    spec = plan.json()["spec"]
    assert spec["jobs"][0]["argv"][1:5] == ["suite", "validate", "acme-api", "--task"]
    assert "--sandbox" in spec["jobs"][0]["argv"] and spec["estimate"]["sessions"] == 0
    monkeypatch.setattr(docker_ps, "docker_available", lambda: False)
    assert c.post(PLANS, json=body).status_code == 409


def test_launch_check_catches_tampering(env: BuilderEnv) -> None:
    plan = _client().post(PLANS, json=_author_body(tasks=["t-one"])).json()
    job = plan["spec"]["jobs"][0]
    assert suites.validate_build_launch(env.cfg, job) == []
    bad = {**job, "argv": [*job["argv"], "--allow-dirty"]}
    assert "argv contains --allow-dirty" in suites.validate_build_launch(env.cfg, bad)
    other = [a if a != "t-one" else "t-two" for a in job["argv"]]
    assert "argv names a different task" in suites.validate_build_launch(
        env.cfg, {**job, "argv": other}
    )
    home = list(job["argv"])
    home[home.index("--suites-home") + 1] = "/tmp/elsewhere"
    assert any(
        "--suites-home" in p for p in suites.validate_build_launch(env.cfg, {**job, "argv": home})
    )
    assert "unexpected env override X" in suites.validate_build_launch(
        env.cfg, {**job, "env": {"X": "1"}}
    )
    (env.cfg.suites_home / "acme-api" / "suite.lock.json").write_text("{}")
    assert "acme-api is frozen" in suites.validate_build_launch(env.cfg, job)


# ----------------------------------------------------------- supervisor --


# The fixture stubs procs.list_processes for the routes; the supervisor needs the real table.
_REAL_LIST = procs.list_processes


def _real_procs() -> list[procs.Proc]:
    """The real process table, limited to this test process and its descendants."""
    procs.reset_cache()
    table = _REAL_LIST()
    me = os.getpid()
    keep = {me} | {p.pid for p in procs.descendants(me, table)}
    return [p for p in table if p.pid in keep]


def _wait(pred: Callable[[], bool], timeout: float = 15.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if pred():
            return
        time.sleep(0.05)
    raise AssertionError("condition not met in time")


def _run_one(
    env: BuilderEnv, body: dict[str, Any], *, tick_until_final: bool = True
) -> tuple[Supervisor, State, int]:
    c = _client()
    plan = c.post(PLANS, json=body).json()
    assert "id" in plan, plan
    job_id = c.post(f"{PLANS}/{plan['id']}/confirm", json={"hash": plan["hash"]}).json()["id"]
    state = State(env.cfg.home / "runner.db")
    sup = Supervisor(env.cfg, state, list_procs=_real_procs, containers=lambda: [])
    if tick_until_final:

        def done() -> bool:
            sup.tick()
            job = state.get_job(job_id)
            return job is not None and not job.active

        _wait(done)
    return sup, state, job_id


@pytest.fixture
def real_ps(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(procs, "list_processes", _real_procs)


@pytest.mark.parametrize(
    ("mode", "status", "note"),
    [
        ("finish", "finished", "authored: 2 accepted, 1 discarded"),
        ("nochange", "failed", "no accepted change"),
        ("refuse", "infra_failed", "refused before the session (exit 3): codex is not ready"),
        ("quota", "failed", "quota window or rate limit"),
        ("crash", "failed", "exited 9"),
    ],
)
def test_author_job_endings(
    env: BuilderEnv, real_ps: None, mode: str, status: str, note: str
) -> None:
    set_fake_suite(env.wt, "author-mode", mode)
    _, state, job_id = _run_one(env, _author_body(tasks=["t-one"]))
    job = state.get_job(job_id)
    assert job is not None and job.status == status, job.notes
    assert any(note in n for n in job.notes), job.notes
    assert len(state.list_jobs()) == 1  # nothing re-queued, nothing retried


def test_author_job_locked_at_launch(env: BuilderEnv, real_ps: None) -> None:
    c = _client()
    plan = c.post(PLANS, json=_author_body(tasks=["t-one"])).json()
    job_id = c.post(f"{PLANS}/{plan['id']}/confirm", json={"hash": plan["hash"]}).json()["id"]
    (env.wt / ".fake-suite-unlocked").unlink()  # locked again between confirm and launch
    state = State(env.cfg.home / "runner.db")
    sup = Supervisor(env.cfg, state, list_procs=_real_procs, containers=lambda: [])

    def done() -> bool:
        sup.tick()
        j = state.get_job(job_id)
        return j is not None and not j.active

    _wait(done)
    job = state.get_job(job_id)
    assert job is not None and job.status == "infra_failed"
    assert "locked" in job.notes[-1]


def test_stopping_an_author_job_kills_the_agent_session(env: BuilderEnv, real_ps: None) -> None:
    set_fake_suite(env.wt, "author-mode", "hang")
    sup, state, job_id = _run_one(env, _author_body(tasks=["t-one"]), tick_until_final=False)
    agent_pid_file = env.wt / ".fake-suite-agent.pid"

    def started() -> bool:
        sup.tick()
        return agent_pid_file.exists()

    _wait(started)
    agent = int(agent_pid_file.read_text())
    state.request_stop(job_id, "test")
    sup.tick()  # SIGINT
    time.sleep(AGENT_KILL_AFTER_S + 0.2)

    def ended() -> bool:
        sup.tick()
        j = state.get_job(job_id)
        return j is not None and not j.active

    _wait(ended)
    job = state.get_job(job_id)
    assert job is not None and job.status == "stopped"
    _wait(lambda: not _alive(agent))


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    try:
        out = os.popen(f"ps -o stat= -p {pid}").read().strip()
    except OSError:
        return True
    return bool(out) and not out.startswith("Z")


@pytest.mark.parametrize(
    ("mode", "status", "note"),
    [
        ("pass", "finished", "verdict: PASS"),
        ("fail", "finished", "verdict: FAIL (base does not fail)"),
        ("infra", "infra_failed", "Docker is not running"),
        ("skip", "failed", "toolchain is missing"),
    ],
)
def test_validate_job_endings(
    env: BuilderEnv, real_ps: None, mode: str, status: str, note: str
) -> None:
    set_fake_suite(env.wt, "validate-mode", mode)
    _, state, job_id = _run_one(env, {"kind": "validate", "suite": "acme-api", "tasks": ["t-two"]})
    job = state.get_job(job_id)
    assert job is not None and job.status == status, job.notes
    assert any(note in n for n in job.notes), job.notes
    stage = {
        t["id"]: t["stage"] for t in _client().get("/api/runner/suites/acme-api").json()["tasks"]
    }
    if mode == "pass":
        assert stage["t-two"] == "validated"
    if mode == "fail":
        assert stage["t-two"] == "validation_failed"


def test_author_job_shows_on_the_runs_page_with_its_console(env: BuilderEnv, real_ps: None) -> None:
    _, _, job_id = _run_one(env, _author_body(tasks=["t-one"]))
    view = _client().get(f"/api/runner/runs/job:{job_id}").json()
    assert view["kind"] == "author" and view["suite_or_task"] == "t-one"
    assert "console.log" in view["log_sources"]
    assert view["job"]["spec"]["task"] == "t-one" and view["job"]["spec"]["pr"] == 1
    log = (
        _client().get(f"/api/runner/runs/job:{job_id}/log", params={"source": "console.log"}).json()
    )
    assert "authoring t-one" in log["text"]

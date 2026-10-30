"""Runner API routes: disabled by default, loopback only, read-only, redacted."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

pytest.importorskip("fastapi")
pytest.importorskip("httpx")

import json

from fastapi.testclient import TestClient

import backend.app as appmod
from backend.runner import docker_ps, launch, procs, sweep
from backend.runner import state as state_mod
from tests.runner_fixtures import (
    SUITE_ID,
    fake_repo,
    fake_worktree,
    gate_job,
    git,
    harness_run,
    make_config,
)


@pytest.fixture
def runner_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    cfg = make_config(tmp_path)
    monkeypatch.setenv("VULCANBENCH_RUNNER", "1")
    monkeypatch.setenv("VULCANBENCH_RUNNER_HOME", str(cfg.home))
    monkeypatch.setenv("VULCANBENCH_RUNNER_GATE_OUT", str(cfg.gate_out))
    monkeypatch.setenv("VULCANBENCH_RUNNER_RUN_WORKTREES", str(cfg.run_worktrees))
    # Keep the tests off the real machine: no ps, no docker.
    monkeypatch.setattr(procs, "list_processes", lambda **_: [])
    monkeypatch.setattr(docker_ps, "list_containers", lambda **_: [])
    gate_job(cfg, "opus-t", task="t", trials=[{"id": "a1", "reward": {"reward": 1}}])
    (cfg.gate_out / "opus-t.log").write_text(
        "harbor done\nAuthorization: Bearer abcdefghijklmnopqrstuvwxyz0123456789\n"
    )
    return tmp_path


def _local() -> TestClient:
    return TestClient(appmod.app, client=("127.0.0.1", 50000))


def test_disabled_by_default(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("VULCANBENCH_RUNNER", raising=False)
    assert _local().get("/api/runner/runs").status_code == 404


def test_non_loopback_clients_are_refused(runner_env: Path) -> None:
    remote = TestClient(appmod.app, client=("10.0.0.7", 1234))
    assert remote.get("/api/runner/runs").status_code == 403
    assert remote.get("/api/runner/status").status_code == 403
    default = TestClient(appmod.app)  # Starlette's default "testclient" host is not loopback
    assert default.get("/api/runner/runs").status_code == 403


def test_list_get_check_and_status(runner_env: Path) -> None:
    c = _local()
    status = c.get("/api/runner/status").json()
    assert status["config"]["enabled"] is True and status["gate_jobs_dir_exists"] is True
    runs = c.get("/api/runner/runs").json()
    assert [r["id"] for r in runs] == ["gate:opus-t"]
    assert runs[0]["solves"] == 1 and "log_sources" not in runs[0]
    assert runs[0]["trials"][0]["reward"]["reward"] == 1
    full = c.get("/api/runner/runs/gate:opus-t").json()
    assert full["trials"][0]["first_check"]["status"] == "ok"
    assert "trial.log" in full["trials"][0]["log_sources"]
    check = c.get("/api/runner/runs/gate:opus-t/check").json()
    assert check["trials"][0]["first_check"]["tool_calls"] == 2
    assert c.get("/api/runner/runs/gate:missing").status_code == 404


def test_log_tail_is_allowlisted_and_redacted(runner_env: Path) -> None:
    c = _local()
    r = c.get("/api/runner/runs/gate:opus-t/log", params={"source": "trial.log", "trial": "a1"})
    assert r.status_code == 200 and r.json()["redacted"] is True
    r = c.get("/api/runner/runs/gate:opus-t/log", params={"source": "../../opus-t.log"})
    assert r.status_code == 200
    assert "Bearer [REDACTED]" in r.json()["text"] and "abcdefghij" not in r.json()["text"]
    for bad in ("agent/sessions/x.jsonl", "../config.json", "/etc/passwd"):
        r = c.get("/api/runner/runs/gate:opus-t/log", params={"source": bad, "trial": "a1"})
        assert r.status_code == 404, bad
    r = c.get("/api/runner/runs/gate:opus-t/log", params={"source": "nope.log"})
    assert r.status_code == 404


# ------------------------------------------------------------- launching --


@pytest.fixture
def launch_env(runner_env: Path, monkeypatch: pytest.MonkeyPatch) -> dict[str, Path]:
    cfg = make_config(runner_env)
    repo = fake_repo(runner_env)
    monkeypatch.setattr(launch, "REPO_ROOT", repo)
    monkeypatch.setattr(sweep, "REPO_ROOT", repo)
    launch._refs_cache.clear()
    wt = fake_worktree(repo, cfg)
    return {"repo": repo, "worktree": wt}


def _plan(c: TestClient, **over: object) -> dict[str, object]:
    body: dict[str, object] = {
        "kind": "gate",
        "ref": "astra",
        "task": "demo-task",
        "tag": "bench/2026-10-29-test",
    }
    body.update(over)
    r = c.post("/api/runner/plans", json=body)
    assert r.status_code == 201, r.text
    data: dict[str, object] = r.json()
    return data


def test_options_and_plan_shape(launch_env: dict[str, Path]) -> None:
    c = _local()
    opts = c.get("/api/runner/options").json()
    assert opts["tags"] == ["bench/2026-10-29-test"] and opts["worktree"]["exists"] is True
    assert [t["name"] for t in opts["tasks"]] == ["demo-task"]
    plan = _plan(c, attempts=2)
    spec = plan["spec"]
    assert isinstance(spec, dict)
    assert spec["argv"][0] == "bash" and spec["argv"][2:] == ["astra", "demo-task"]
    assert spec["env"] == {
        "GATE_OUT": str(make_config(launch_env["repo"].parent).gate_out),
        "GATE_ATTEMPTS": "2",
    }
    assert plan["live_reason"] is None and plan["usage_check_ok"] is True
    assert "CLAUDE_CODE_OAUTH_TOKEN" not in r_text(plan)


def r_text(obj: object) -> str:
    return json.dumps(obj)


def test_plan_refusals_are_http_errors(launch_env: dict[str, Path]) -> None:
    c = _local()
    r = c.post(
        "/api/runner/plans",
        json={"kind": "gate", "ref": "ultra", "task": "demo-task", "tag": "bench/2026-10-29-test"},
    )
    assert r.status_code == 422
    r = c.post(
        "/api/runner/plans",
        json={"kind": "suite", "ref": "astra", "task": "demo-task", "tag": "bench/2026-10-29-test"},
    )
    assert r.status_code == 422
    r = c.post(
        "/api/runner/plans",
        json={
            "kind": "gate",
            "ref": "astra",
            "task": "demo-task",
            "tag": "bench/2026-10-29-test",
            "attempts": 9,
        },
    )
    assert r.status_code == 422
    assert c.get("/api/runner/plans/nope").status_code == 404


def test_confirm_needs_the_hash_and_a_live_plan(
    launch_env: dict[str, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    c = _local()
    plan = _plan(c)
    pid = str(plan["id"])
    assert c.post(f"/api/runner/plans/{pid}/confirm", json={"hash": "0" * 16}).status_code == 409
    r = c.post(f"/api/runner/plans/{pid}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 201, r.text
    job = r.json()
    assert (
        job["status"] == "queued"
        and job["name"] == "astra-demo-task"
        and job["spec"]["argv"][2:] == ["astra", "demo-task"]
    )
    assert (
        c.post(f"/api/runner/plans/{pid}/confirm", json={"hash": plan["hash"]}).status_code == 409
    )
    # The queue and the Runs page both show it; nothing started (no supervisor in tests).
    q = c.get("/api/runner/queue").json()
    assert [j["id"] for j in q["jobs"]] == [job["id"]] and q["supervisor"]["status"] in {
        "disabled",
        "stopped",
        "inactive",
    }
    runs = c.get("/api/runner/runs").json()
    row = next(r for r in runs if r["id"] == f"job:{job['id']}")
    assert (
        row["status"] == "queued"
        and row["owned"] is True
        and row["source_tag"] == "bench/2026-10-29-test"
    )
    full = c.get(f"/api/runner/runs/job:{job['id']}").json()
    assert full["job"]["id"] == job["id"] and full["trials"] == []
    # An expired plan cannot be confirmed.
    monkeypatch.setattr(state_mod, "PLAN_TTL_S", 0)
    stale = _plan(c, suffix="-b")
    r = c.post(f"/api/runner/plans/{stale['id']}/confirm", json={"hash": stale["hash"]})
    assert r.status_code == 409 and "expired" in r.json()["detail"]


def test_claude_refs_need_a_fresh_usage_check(
    launch_env: dict[str, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    c = _local()
    plan = _plan(c, ref="opus")
    pid = str(plan["id"])
    assert plan["usage_check_ok"] is False
    r = c.post(f"/api/runner/plans/{pid}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 409 and "usage check" in r.json()["detail"]
    calls: list[str] = []

    def fake_preflight(_cfg: object, spec: dict[str, object], **_: object) -> dict[str, object]:
        calls.append(str(spec["ref"]))
        return {"status": "allowed", "max_utilization": 0.3, "would_start": True}

    monkeypatch.setattr(launch, "run_usage_preflight", fake_preflight)
    checked = c.post(f"/api/runner/plans/{pid}/usage").json()
    assert (
        calls == ["opus"]
        and checked["usage_check"]["would_start"] is True
        and checked["usage_check_ok"] is True
    )
    r = c.post(f"/api/runner/plans/{pid}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 201, r.text
    # A refused pre-flight blocks confirm.
    monkeypatch.setattr(
        launch,
        "run_usage_preflight",
        lambda *_a, **_k: {"status": "allowed", "max_utilization": 0.9, "would_start": False},
    )
    plan2 = _plan(c, ref="opus", suffix="-b")
    c.post(f"/api/runner/plans/{plan2['id']}/usage")
    assert (
        c.post(f"/api/runner/plans/{plan2['id']}/confirm", json={"hash": plan2["hash"]}).status_code
        == 409
    )


def test_confirm_revalidates_the_worktree(launch_env: dict[str, Path]) -> None:
    c = _local()
    plan = _plan(c)
    (launch_env["worktree"] / "tasks" / "frontier-v5" / "demo-task" / "task.toml").write_text(
        "changed\n"
    )
    r = c.post(f"/api/runner/plans/{plan['id']}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 409 and "uncommitted changes" in r.json()["detail"]


def test_queue_stop_and_remove(launch_env: dict[str, Path]) -> None:
    c = _local()
    a = c.post(
        f"/api/runner/plans/{_plan(c)['id']}/confirm", json={"hash": _plan(c, suffix="-x")["hash"]}
    )
    assert a.status_code == 409  # hash of a different plan
    plan = _plan(c)
    job = c.post(f"/api/runner/plans/{plan['id']}/confirm", json={"hash": plan["hash"]}).json()
    jid = job["id"]
    got = c.get(f"/api/runner/jobs/{jid}").json()
    assert got["status"] == "queued" and got["console"] == ""
    stopped = c.post(f"/api/runner/jobs/{jid}/stop", json={"reason": "changed my mind"}).json()
    assert stopped["status"] == "stopped" and stopped["stop_reason"] == "changed my mind"
    assert c.post(f"/api/runner/jobs/{jid}/stop").status_code == 409
    assert c.delete(f"/api/runner/jobs/{jid}").status_code == 409
    plan2 = _plan(c, suffix="-again")
    job2 = c.post(f"/api/runner/plans/{plan2['id']}/confirm", json={"hash": plan2["hash"]}).json()
    assert c.delete(f"/api/runner/jobs/{job2['id']}").status_code == 204
    assert c.get(f"/api/runner/jobs/{job2['id']}").status_code == 404
    assert c.get("/api/runner/jobs/999").status_code == 404


# ------------------------------------------------------- benchmark a model --


def _sweep_plan(c: TestClient, **over: object) -> dict[str, Any]:
    body: dict[str, object] = {
        "kind": "sweep",
        "model": "gpt-6.1-terra",
        "harness": "codex",
        "suite": SUITE_ID,
        "tag": "bench/2026-10-29-test",
        "efforts": ["low", "max"],
        "billing": "subscription",
    }
    body.update(over)
    r = c.post("/api/runner/plans", json=body)
    assert r.status_code == 201, r.text
    data: dict[str, Any] = r.json()
    return data


def test_options_cover_the_model_sweep(launch_env: dict[str, Path]) -> None:
    opts = _local().get("/api/runner/options").json()
    assert opts["kinds"] == ["sweep", "gate", "prepare"]
    assert [s["id"] for s in opts["suites"]] == [SUITE_ID] and opts["suites"][0]["n_tasks"] == 2
    assert "ultra" not in opts["efforts"] and opts["efforts"][0] == "minimal"
    assert "codex" in opts["harnesses"] and opts["sandbox_defaults"]["codex"] == "local"
    assert opts["quota_wait_s"] == 1800


def test_sweep_plan_confirm_queue_and_progress(launch_env: dict[str, Path]) -> None:
    c = _local()
    plan = _sweep_plan(c)
    spec = plan["spec"]
    assert spec["kind"] == "sweep" and [j["effort"] for j in spec["jobs"]] == ["low", "max"]
    assert spec["doctor"]["status"] == "ready" and spec["estimate"]["total_usd"] == 9.0
    assert plan["usage_check_ok"] is True and plan["live_reason"] is None
    r = c.post(f"/api/runner/plans/{plan['id']}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 201, r.text
    first = r.json()
    assert first["kind"] == "suite" and [j["name"] for j in first["jobs"]] == [
        f"{spec['name']}-low",
        f"{spec['name']}-max",
    ]
    q = c.get("/api/runner/queue").json()
    assert [j["status"] for j in q["jobs"]] == ["queued", "queued"]
    # Progress from disk: nothing yet, the level rows show 0 of 2.
    sweeps = c.get("/api/runner/sweeps").json()["sweeps"]
    assert len(sweeps) == 1 and sweeps[0]["model"] == "gpt-6.1-terra"
    levels = sweeps[0]["levels"]
    assert [lv["job"]["spec"]["effort"] for lv in levels] == ["low", "max"]
    assert levels[0]["progress"]["total_runs"] == 2 and sweeps[0]["totals"]["active"] is True
    runs = c.get("/api/runner/runs").json()
    rows = [r for r in runs if r["id"].startswith("job:")]
    assert {r["effort"] for r in rows} == {"low", "max"} and rows[0]["attempts_total"] == 2
    assert rows[0]["kind"] == "suite" and rows[0]["suite_or_task"] == SUITE_ID
    # A finished run under the level's directory is owned and counted.
    out_dir = Path(levels[0]["progress"]["out_dir"])
    harness_run(
        out_dir, "alpha-task-aaaaaaaa", task_id="alpha-task", functional=1.0, suite=SUITE_ID
    )
    harness_run(out_dir, "beta-task-bbbbbbbb", task_id="beta-task", finished=False)
    sweeps = c.get("/api/runner/sweeps").json()["sweeps"]
    prog = sweeps[0]["levels"][0]["progress"]
    assert (
        prog["runs_done"] == 1 and prog["passed"] == 1 and prog["current"]["task_id"] == "beta-task"
    )
    assert prog["current"]["run_id_href"].startswith("harness:0:")
    run = c.get(f"/api/runner/runs/{prog['current']['run_id_href']}").json()
    assert run["owned"] is True and run["job"]["id"] == levels[0]["job"]["id"]
    # The job is queued, not running, so a trace without a summary is a stopped run.
    assert run["status"] == "stopped"
    # Removing a queued level and stopping the other.
    ids = [lv["job"]["id"] for lv in levels]
    assert c.delete(f"/api/runner/jobs/{ids[1]}").status_code == 204
    assert c.post(f"/api/runner/jobs/{ids[0]}/stop").json()["status"] == "stopped"


def test_sweep_plan_refusals_and_doctor_gate(launch_env: dict[str, Path]) -> None:
    c = _local()
    r = c.post(
        "/api/runner/plans",
        json={
            "kind": "sweep",
            "model": "x",
            "harness": "codex",
            "suite": SUITE_ID,
            "tag": "bench/2026-10-29-test",
            "efforts": ["ultra"],
        },
    )
    assert r.status_code == 422 and "blocked" in r.text
    r = c.post(
        "/api/runner/plans",
        json={
            "kind": "sweep",
            "model": "x",
            "harness": "codex",
            "suite": SUITE_ID,
            "tag": "bench/2026-10-29-test",
            "efforts": ["low"],
            "quota_continue": 999,
        },
    )
    assert r.status_code == 422
    (launch_env["worktree"] / ".fake-doctor-not-ready").write_text("")
    plan = _sweep_plan(c, efforts=["low"])
    assert plan["spec"]["doctor"]["status"] == "not_ready"
    r = c.post(f"/api/runner/plans/{plan['id']}/confirm", json={"hash": plan["hash"]})
    assert r.status_code == 409 and "harness not ready" in r.json()["detail"]


def test_prepare_plan_is_a_quota_free_queue_step(launch_env: dict[str, Path]) -> None:
    c = _local()
    tag = "bench/2026-10-29-test"
    r = c.post("/api/runner/plans", json={"kind": "prepare", "tag": tag})
    assert r.status_code == 409 and "already exists" in r.json()["detail"]
    git(launch_env["repo"], "tag", "bench/2026-10-30-new")
    plan = c.post(
        "/api/runner/plans", json={"kind": "prepare", "tag": "bench/2026-10-30-new"}
    ).json()
    spec = plan["spec"]
    assert (
        spec["kind"] == "prepare"
        and spec["argv"] is None
        and spec["prep_argv"][2] == "bench/2026-10-30-new"
    )
    assert "no model calls" in spec["estimate"]["billing"]
    job = c.post(f"/api/runner/plans/{plan['id']}/confirm", json={"hash": plan["hash"]}).json()
    assert job["kind"] == "prepare" and job["status"] == "queued"
    row = next(r for r in c.get("/api/runner/runs").json() if r["id"] == f"job:{job['id']}")
    assert row["kind"] == "prepare" and row["source_tag"] == "bench/2026-10-30-new"

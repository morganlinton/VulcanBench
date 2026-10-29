"""Runner API routes: disabled by default, loopback only, read-only, redacted."""

from __future__ import annotations

from pathlib import Path

import pytest

pytest.importorskip("fastapi")
pytest.importorskip("httpx")

from fastapi.testclient import TestClient

import backend.app as appmod
from backend.runner import docker_ps, procs
from tests.runner_fixtures import gate_job, make_config


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

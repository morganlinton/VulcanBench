"""The Launch page's readiness checklist: read-only, quota-free, no credential output."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

pytest.importorskip("fastapi")

from fastapi.testclient import TestClient

from backend import app as appmod
from backend.runner import docker_ps, launch, readiness
from tests.runner_fixtures import (
    BUILDER_TAG,
    builder_env,
    fake_repo,
    make_config,
    set_fake_suite,
)


def _client() -> TestClient:
    return TestClient(appmod.app, client=("127.0.0.1", 50000))


def _by_id(doc: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {c["id"]: c for c in doc["checks"]}


@pytest.fixture
def no_gh(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(readiness.shutil, "which", lambda name: None)


def test_ready_builder_mac(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, no_gh: None) -> None:
    env = builder_env(tmp_path, monkeypatch)
    set_fake_suite(env.wt, "unlocked")
    monkeypatch.setattr(docker_ps, "docker_available", lambda: True)
    res = _client().get("/api/runner/readiness")
    assert res.status_code == 200
    checks = _by_id(res.json())
    assert checks["bench_tag"]["status"] == "ok" and BUILDER_TAG in checks["bench_tag"]["detail"]
    assert checks["docker"]["status"] == "ok"
    assert checks["builder"]["status"] == "ok" and BUILDER_TAG in checks["builder"]["detail"]
    assert checks["gh"]["status"] == "warn" and "not installed" in checks["gh"]["detail"]
    assert res.json()["status"] in {"warn", "fail"}  # gh missing, supervisor off in tests


def test_locked_and_unconfigured_builder(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, no_gh: None
) -> None:
    env = builder_env(tmp_path, monkeypatch)
    cfg = env.cfg
    locked = readiness.builder_check(cfg)
    assert locked["status"] == "ok" and "locked" in locked["detail"]
    set_fake_suite(env.wt, "not-configured")
    check = readiness.builder_check(cfg)
    assert check["status"] == "warn" and "no password" in check["detail"]
    assert "password-hash" in (check["fix"] or "")


def test_unprepared_newest_tag(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", repo)
    cfg = make_config(tmp_path)
    check = readiness.tag_check(cfg)
    assert check["status"] == "warn" and "no bench tag has a run worktree" in check["detail"]
    builder = readiness.builder_check(cfg)
    assert builder["status"] == "warn" and "no prepared bench tag" in builder["detail"]


def test_no_tags(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(launch, "bench_tags", lambda repo=None: [])
    check = readiness.tag_check(make_config(tmp_path))
    assert check["status"] == "fail" and "Cut" in (check["fix"] or "")


def test_supervisor_states() -> None:
    assert readiness.supervisor_check({"status": "active"})["status"] == "ok"
    held = readiness.supervisor_check({"status": "active", "hold_reason": "quota window"})
    assert held["status"] == "fail" and "quota window" in held["detail"]
    assert readiness.supervisor_check({"status": "disabled"})["status"] == "fail"


def test_gh_output_is_never_read(monkeypatch: pytest.MonkeyPatch) -> None:
    seen: dict[str, Any] = {}

    def fake_run(argv: list[str], **kw: Any) -> subprocess.CompletedProcess[bytes]:
        seen.update(kw, argv=argv)
        return subprocess.CompletedProcess(argv, 0)

    monkeypatch.setattr(readiness.shutil, "which", lambda name: "/opt/homebrew/bin/gh")
    monkeypatch.setattr(readiness.subprocess, "run", fake_run)
    monkeypatch.setenv("GH_TOKEN", "ghp_should_not_pass_through")
    check = readiness.gh_check()
    assert check["status"] == "ok"
    assert seen["argv"][1:3] == ["auth", "status"]
    assert seen["stdout"] is subprocess.DEVNULL and seen["stderr"] is subprocess.DEVNULL
    assert seen["stdin"] is subprocess.DEVNULL
    assert "GH_TOKEN" not in seen["env"] and seen["env"]["GH_PROMPT_DISABLED"] == "1"

    monkeypatch.setattr(
        readiness.subprocess, "run", lambda argv, **kw: subprocess.CompletedProcess(argv, 1)
    )
    assert readiness.gh_check()["status"] == "warn"

    def hang(argv: list[str], **kw: Any) -> None:
        raise subprocess.TimeoutExpired(argv, kw["timeout"])

    monkeypatch.setattr(readiness.subprocess, "run", hang)
    assert "did not answer" in readiness.gh_check()["detail"]

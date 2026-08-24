"""Tests for multi-service docker-compose task environments."""

from __future__ import annotations

import json
import socket
from pathlib import Path

import pytest

from harness.environment import TaskEnvironment, environment_spec, start_environment
from harness.sandbox.docker_executor import SandboxError
from harness.task_metadata import _validate_environment
from harness.tasks import load_task

DEMO_ROOT = Path(__file__).resolve().parents[1] / "tasks" / "cii-v2"
DEMO_ID = "demo-compose-redis-smoke"


def test_environment_spec_parsing():
    assert environment_spec({}) is None
    assert environment_spec({"environment": {}}) is None
    assert environment_spec({"environment": {"compose": "c.yaml"}}) == {"compose": "c.yaml"}


def test_validate_environment_rejects_footguns(tmp_path: Path):
    (tmp_path / "compose.yaml").write_text(
        'services:\n  db:\n    image: redis\n    container_name: fixed\n    ports:\n      - "6379:6379"\n'
    )
    reasons = _validate_environment(
        tmp_path, {"environment": {"compose": "compose.yaml", "ready": [{"service": "db"}]}}
    )
    text = " ".join(reasons)
    assert "container_name" in text
    assert "ephemerally" in text
    assert "malformed" in text


def test_validate_environment_accepts_clean_spec(tmp_path: Path):
    (tmp_path / "compose.yaml").write_text(
        'services:\n  db:\n    image: redis\n    ports:\n      - "6379"\n'
    )
    spec = {
        "environment": {
            "compose": "compose.yaml",
            "ready": [{"service": "db", "cmd": "true"}],
            "up_timeout_s": 60,
        }
    }
    assert _validate_environment(tmp_path, spec) == []


def test_validate_environment_missing_file(tmp_path: Path):
    reasons = _validate_environment(tmp_path, {"environment": {"compose": "nope.yaml"}})
    assert reasons and "not found" in reasons[0]


def test_start_environment_none_without_spec(tmp_path: Path):
    task = load_task(DEMO_ID, DEMO_ROOT)
    task.metadata.pop("environment")
    assert start_environment(task, "t", tmp_path) is None


def test_start_environment_rejects_docker_sandbox(tmp_path: Path):
    task = load_task(DEMO_ID, DEMO_ROOT)
    with pytest.raises(SandboxError, match="sandbox local"):
        start_environment(task, "t", tmp_path, sandbox="docker")


@pytest.mark.docker
def test_environment_full_lifecycle(tmp_path: Path):
    task = load_task(DEMO_ID, DEMO_ROOT)
    env = start_environment(task, "pytest-lifecycle", tmp_path)
    assert env is not None
    try:
        assert env.project.startswith("vb-")
        assert "redis" in env.ports and "6379" in env.ports["redis"]
        manifest = json.loads((tmp_path / ".vb_services.json").read_text())
        assert manifest["project"] == env.project
        assert (tmp_path / ".gitignore").read_text().count(".vb_services.json") == 1
        # ready probe already ran; the port answers PING
        with socket.create_connection(("127.0.0.1", env.ports["redis"]["6379"]), timeout=5) as s:
            s.sendall(b"*1\r\n$4\r\nPING\r\n")
            assert s.recv(64).startswith(b"+PONG")
    finally:
        env.down()
    # down is idempotent
    env.down()


@pytest.mark.docker
def test_environment_up_failure_is_sandbox_error(tmp_path: Path):
    bad = tmp_path / "bad.yaml"
    bad.write_text("services:\n  x:\n    image: vulcanbench/definitely-not-an-image:zzz\n")
    env = TaskEnvironment(bad, "vb-pytest-badimage", {"up_timeout_s": 30})
    with pytest.raises(SandboxError):
        env.up()
    env.down()

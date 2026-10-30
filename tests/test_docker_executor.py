"""Tests for the Docker sandbox executor.

The Docker daemon is not assumed to be running, so these replace the module's
``_run_docker`` seam (the one place the executor shells out to the ``docker``
CLI) with a fake that records every argument vector and scripts the replies.
Two live tests are gated on ``_docker_available()`` and skipped otherwise.
"""

from __future__ import annotations

import os
from collections.abc import Callable
from pathlib import Path

import pytest

from harness.agent.loop import _collect_manifest, _executor_runner, _executor_runner_outcome
from harness.agent.protocol import EditFileArgs, ReadFileArgs, RunCommandArgs
from harness.sandbox import docker_executor
from harness.sandbox.docker_executor import (
    _SANDBOX_ENV,
    DEFAULT_IMAGE,
    DockerResult,
    DockerToolExecutor,
    ResourceSpec,
    SandboxError,
    _docker_available,
)
from harness.verifier import VerifierInfrastructureError

CONTAINER_ID = "cafebabe0001"
_OK = DockerResult(0, b"", b"")


def _default_exec(cmd: list[str]) -> DockerResult:
    return DockerResult(0, b"hi\n", b"")


class FakeDocker:
    """Stands in for the ``docker`` CLI: records argv, scripts the replies."""

    def __init__(self, *, daemon_up: bool = True) -> None:
        self.daemon_up = daemon_up
        self.calls: list[list[str]] = []
        self.exec_reply: Callable[[list[str]], DockerResult] = _default_exec

    def __call__(self, args: list[str], *, timeout: float | None = None) -> DockerResult:
        self.calls.append(list(args))
        verb = args[0]
        if verb == "version":
            if self.daemon_up:
                return DockerResult(0, b"27.0.0\n", b"")
            return DockerResult(1, b"", b"Cannot connect to the Docker daemon at unix:///x\n")
        if verb == "run":
            return DockerResult(0, f"{CONTAINER_ID}\n".encode(), b"")
        if verb == "exec":
            assert CONTAINER_ID in args, args
            return self.exec_reply(args[args.index(CONTAINER_ID) + 1 :])
        if verb in {"stop", "rm"}:
            return _OK
        raise AssertionError(f"unexpected docker verb: {args}")

    @property
    def run_argv(self) -> list[str]:
        return next(c for c in self.calls if c[0] == "run")

    @property
    def exec_argvs(self) -> list[list[str]]:
        return [c for c in self.calls if c[0] == "exec"]

    @property
    def exec_cmds(self) -> list[list[str]]:
        """The command vectors (after the container id) of every exec."""
        return [c[c.index(CONTAINER_ID) + 1 :] for c in self.exec_argvs]

    @property
    def stopped(self) -> bool:
        return any(c[0] == "stop" for c in self.calls)

    @property
    def removed(self) -> bool:
        return any(c[0] == "rm" for c in self.calls)


def _opt(argv: list[str], flag: str) -> str:
    """The value following ``flag`` in ``argv`` (first occurrence)."""
    return argv[argv.index(flag) + 1]


def _opts(argv: list[str], flag: str) -> list[str]:
    return [argv[i + 1] for i, a in enumerate(argv) if a == flag]


@pytest.fixture
def fake_docker(monkeypatch: pytest.MonkeyPatch) -> FakeDocker:
    fake = FakeDocker()
    monkeypatch.setattr(docker_executor, "_run_docker", fake)
    return fake


def test_construction_argv(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path, image="img:test", network=False, cpus=2.0)
    argv = fake_docker.run_argv
    assert argv[:2] == ["run", "--detach"]
    assert argv[-3:] == ["img:test", "sleep", "infinity"]
    assert _opt(argv, "--workdir") == "/workspace"
    assert _opt(argv, "--volume") == f"{tmp_path.resolve()}:/workspace:rw"
    assert _opt(argv, "--network") == "none"
    assert _opt(argv, "--cpus") == "2.0"
    assert _opt(argv, "--pids-limit") == "512"
    assert _opt(argv, "--user") == f"{os.getuid()}:{os.getgid()}"
    assert _opts(argv, "--env") == [f"{k}={v}" for k, v in _SANDBOX_ENV.items()]
    assert _opt(argv, "--security-opt") == "no-new-privileges"
    assert _opt(argv, "--cap-drop") == "ALL"
    assert ex.container_id == CONTAINER_ID
    ex.close()


def test_default_image_is_all_language_base() -> None:
    if os.environ.get("VULCANBENCH_SANDBOX_IMAGE"):
        pytest.skip("default image overridden by environment")
    assert DEFAULT_IMAGE == "vulcanbench/sandbox:base"


def test_network_enabled_flag(tmp_path: Path, fake_docker: FakeDocker) -> None:
    DockerToolExecutor(tmp_path, network=True).close()
    assert "--network" not in fake_docker.run_argv


def test_run_command_execs_in_container(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path, image="img")
    out = ex.run_command(RunCommandArgs(cmd="echo hi"))
    assert out == {"stdout": "hi\n", "stderr": "", "exit_code": 0}
    argv = fake_docker.exec_argvs[-1]
    assert argv[:3] == ["exec", "--workdir", "/workspace"]
    cmd = fake_docker.exec_cmds[-1]
    assert cmd[:2] == ["sh", "-c"]
    assert "timeout" in cmd[2]
    assert "echo hi" in cmd[2]
    ex.close()


def test_run_tests_and_lint_exec(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path)
    ex.run_tests()
    ex.run_lint()
    cmds = [c[2] for c in fake_docker.exec_cmds]
    assert any("pytest" in c for c in cmds)
    assert any("ruff" in c for c in cmds)
    assert ex.run_build() == {"ok": True}
    ex.close()


def test_executor_runner_execs_in_container(tmp_path: Path, fake_docker: FakeDocker) -> None:
    """The in-container verifier runner routes test commands through docker exec."""
    ex = DockerToolExecutor(tmp_path)
    runner = _executor_runner(ex)
    exit_code = runner("pytest test_x.py -q", tmp_path, 60)
    assert exit_code == 0  # the fake exec returns exit code 0
    assert "pytest test_x.py -q" in fake_docker.exec_cmds[-1][2]
    ex.close()


def test_manifest_versions_are_collected_in_container(
    tmp_path: Path, fake_docker: FakeDocker
) -> None:
    ex = DockerToolExecutor(tmp_path)
    manifest = _collect_manifest("mock:model", None, False, ex)
    assert manifest["sandbox"]["mode"] == "docker"
    assert manifest["tools"]["git"] == "hi"
    assert manifest["runtime"]["python"] == "hi"
    cmds = "\n".join(c[2] for c in fake_docker.exec_cmds)
    assert "git --version" in cmds
    assert "go version" in cmds
    assert "node --version" in cmds
    ex.close()


def test_file_ops_delegate_to_host(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path)
    ex.edit_file(EditFileArgs(path="a.txt", old_string="", new_string="data\n"))
    # File written on the host side (the shared bind mount), no container exec.
    assert (tmp_path / "a.txt").read_text() == "data\n"
    assert ex.read_file(ReadFileArgs(path="a.txt")) == "data\n"
    assert fake_docker.exec_argvs == []
    ex.close()


def test_close_stops_and_removes(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path)
    ex.close()
    assert fake_docker.stopped and fake_docker.removed
    stop = next(c for c in fake_docker.calls if c[0] == "stop")
    assert stop == ["stop", "--time", "5", CONTAINER_ID]
    assert ["rm", "--force", CONTAINER_ID] in fake_docker.calls
    ex.close()  # idempotent: no second stop/rm
    assert sum(c[0] in {"stop", "rm"} for c in fake_docker.calls) == 2


def test_context_manager(tmp_path: Path, fake_docker: FakeDocker) -> None:
    with DockerToolExecutor(tmp_path) as ex:
        assert ex is not None
    assert fake_docker.removed


def test_daemon_down_raises(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(docker_executor, "_run_docker", FakeDocker(daemon_up=False))
    with pytest.raises(SandboxError, match="Docker daemon"):
        DockerToolExecutor(tmp_path)


def test_missing_docker_binary_raises(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    def no_binary(args: list[str], *, timeout: float | None = None) -> DockerResult:
        raise FileNotFoundError("docker")

    monkeypatch.setattr(docker_executor, "_run_docker", no_binary)
    with pytest.raises(SandboxError, match="Docker daemon"):
        DockerToolExecutor(tmp_path)


def test_failed_start_raises_with_daemon_detail(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fake = FakeDocker()

    def run_fails(args: list[str], *, timeout: float | None = None) -> DockerResult:
        if args[0] == "run":
            return DockerResult(125, b"", b"Unable to find image 'nope:latest' locally\n")
        return fake(args, timeout=timeout)

    monkeypatch.setattr(docker_executor, "_run_docker", run_fails)
    with pytest.raises(SandboxError, match="nope:latest"):
        DockerToolExecutor(tmp_path, image="nope:latest")


def test_daemon_error_during_exec_is_sandbox_error(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path, image="img")
    fake_docker.exec_reply = lambda cmd: DockerResult(
        125, b"", b"Error response from daemon: No such container: cafebabe0001\n"
    )
    with pytest.raises(SandboxError, match="No such container"):
        ex.run_command(RunCommandArgs(cmd="echo hi"))
    ex.close()


def test_command_exit_125_without_daemon_text_is_the_commands_own(
    tmp_path: Path, fake_docker: FakeDocker
) -> None:
    ex = DockerToolExecutor(tmp_path, image="img")
    fake_docker.exec_reply = lambda cmd: DockerResult(125, b"", b"my tool failed\n")
    out = ex.run_command(RunCommandArgs(cmd="mytool"))
    assert out["exit_code"] == 125
    assert out["stderr"] == "my tool failed\n"
    ex.close()


def test_docker_available_false_when_unreachable(monkeypatch: pytest.MonkeyPatch) -> None:
    def boom(args: list[str], *, timeout: float | None = None) -> DockerResult:
        raise OSError("down")

    monkeypatch.setattr(docker_executor, "_run_docker", boom)
    assert _docker_available() is False
    monkeypatch.setattr(docker_executor, "_run_docker", FakeDocker(daemon_up=False))
    assert _docker_available() is False


def test_docker_available_true(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(docker_executor, "_run_docker", FakeDocker())
    assert _docker_available() is True


@pytest.mark.docker
@pytest.mark.skipif(not _docker_available(), reason="requires a running Docker daemon")
def test_live_go_test_without_gocache_export(tmp_path: Path) -> None:
    """Go verification must work when the agent does not export GOCACHE."""
    if os.environ.get("VULCANBENCH_SANDBOX_IMAGE"):
        image = os.environ["VULCANBENCH_SANDBOX_IMAGE"]
    else:
        image = DEFAULT_IMAGE
    ws = tmp_path / "goproj"
    (ws / "stack").mkdir(parents=True)
    (ws / "go.mod").write_text("module example.com/stackdemo\n\ngo 1.23\n")
    (ws / "stack" / "stack.go").write_text(
        "package stack\n\ntype Stack struct{ items []int }\n"
        "func New() *Stack { return &Stack{} }\n"
        "func (s *Stack) Push(v int) { s.items = append(s.items, v) }\n"
        "func (s *Stack) Len() int { return len(s.items) }\n"
    )
    (ws / "stack" / "pop.go").write_text(
        "package stack\n\nfunc (s *Stack) Pop() (int, bool) {\n"
        "  if len(s.items) == 0 { return 0, false }\n"
        "  i := len(s.items) - 1\n  v := s.items[i]\n"
        "  s.items = s.items[:i]\n  return v, true\n}\n"
    )
    (ws / "stack" / "pop_test.go").write_text(
        'package stack\n\nimport "testing"\n\n'
        "func TestPopLIFO(t *testing.T) {\n"
        "  s := New()\n  s.Push(1)\n  s.Push(2)\n"
        "  v, ok := s.Pop()\n"
        '  if !ok || v != 2 { t.Fatalf("got %d,%v want 2,true", v, ok) }\n'
        "}\n"
    )
    with DockerToolExecutor(ws, image=image) as ex:
        runner = _executor_runner(ex)
        assert runner("go test -run '^TestPopLIFO$' ./...", ws, 120) == 0


@pytest.mark.docker
@pytest.mark.skipif(not _docker_available(), reason="requires a running Docker daemon")
def test_live_run_command(tmp_path: Path) -> None:
    with DockerToolExecutor(tmp_path, image="python:3.12-slim") as ex:
        out = ex.run_command(RunCommandArgs(cmd="echo sandbox-ok"))
        assert out["exit_code"] == 0
        assert "sandbox-ok" in out["stdout"]


# --- resource band (floor/ceiling) and OOM classification ---------------------


def test_resource_spec_floor_ceiling_mapping(tmp_path: Path, fake_docker: FakeDocker) -> None:
    spec = ResourceSpec(
        mem_floor="1g", mem_ceiling="4g", cpu_floor=1.0, cpu_ceiling=3.0, pids_limit=1024
    )
    ex = DockerToolExecutor(tmp_path, image="img", resources=spec)
    argv = fake_docker.run_argv
    assert _opt(argv, "--memory-reservation") == "1g"
    assert _opt(argv, "--memory") == "4g"
    assert _opt(argv, "--cpu-shares") == "1024"
    assert _opt(argv, "--cpus") == "3.0"
    assert _opt(argv, "--pids-limit") == "1024"
    assert ex.resources is spec
    ex.close()


def test_legacy_kwargs_become_floorless_band(tmp_path: Path, fake_docker: FakeDocker) -> None:
    ex = DockerToolExecutor(tmp_path, mem_limit="3g", cpus=1.5, pids_limit=256)
    argv = fake_docker.run_argv
    assert _opt(argv, "--memory") == "3g"
    assert _opt(argv, "--cpus") == "1.5"
    assert _opt(argv, "--pids-limit") == "256"
    assert "--memory-reservation" not in argv
    assert "--cpu-shares" not in argv
    assert ex.resources == ResourceSpec(mem_ceiling="3g", cpu_ceiling=1.5, pids_limit=256)
    ex.close()


class OOMReplies:
    """SIGKILL exits for commands and a live oom_kill counter for cgroup reads."""

    def __init__(self) -> None:
        self.oom_kills = 0

    def __call__(self, cmd: list[str]) -> DockerResult:
        shell = cmd[-1]
        if "memory.events" in shell:
            return DockerResult(0, f"low 0\noom 0\noom_kill {self.oom_kills}\n".encode(), b"")
        return DockerResult(137, b"", b"Killed\n")


def test_oom_kill_counted_only_when_counter_moves(tmp_path: Path, fake_docker: FakeDocker) -> None:
    replies = OOMReplies()
    fake_docker.exec_reply = replies
    ex = DockerToolExecutor(tmp_path, image="img")
    # SIGKILL exit with a flat counter: a plain kill, not an OOM.
    out = ex.run_command(RunCommandArgs(cmd="sleep 999"))
    assert out["exit_code"] == 137
    assert ex.oom_kill_count == 0
    # Counter moves: the next 137 is attributed to the memory ceiling.
    replies.oom_kills = 1
    ex.run_command(RunCommandArgs(cmd="hog-memory"))
    assert ex.oom_kill_count == 1
    ex.close()


def test_verifier_oom_raises_infrastructure_error(tmp_path: Path, fake_docker: FakeDocker) -> None:
    replies = OOMReplies()
    fake_docker.exec_reply = replies
    ex = DockerToolExecutor(tmp_path, image="img")
    replies.oom_kills = 2
    with pytest.raises(VerifierInfrastructureError, match="OOM"):
        _executor_runner_outcome(ex, "pytest -q", 120)
    ex.close()


def test_verifier_plain_sigkill_is_not_infrastructure(
    tmp_path: Path, fake_docker: FakeDocker
) -> None:
    fake_docker.exec_reply = OOMReplies()
    ex = DockerToolExecutor(tmp_path, image="img")
    outcome = _executor_runner_outcome(ex, "pytest -q", 120)
    assert outcome.exit_code == 137
    assert ex.oom_kill_count == 0
    ex.close()

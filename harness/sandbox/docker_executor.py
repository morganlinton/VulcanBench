"""Docker-backed tool executor.

Runs the agent's *code execution* (``run_command`` and friends) inside an
isolated, non-root, network-off, resource-limited container, while delegating
*file* operations to a host-side :class:`LocalToolExecutor` over the bind-mounted
workspace. Splitting it this way keeps file semantics identical to local runs
(same bytes, same path-escape guard) and confines the only untrusted thing -- the
shell commands a model asks to run -- to the container.

The workspace is bind-mounted at ``/workspace`` and the container runs as the
host UID/GID so files created inside stay writable and owned by the host user
(and never root). The container is detached and long-lived (``sleep infinity``);
each tool call is a ``docker exec`` into it. Always ``close()`` it (the agent
loop does so in a ``finally``).

Everything goes through the ``docker`` CLI (``_run_docker``), the same binary
the CLI-agent adapters already drive, so the harness needs no Docker SDK.
"""

from __future__ import annotations

import contextlib
import os
import shlex
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any

from harness.agent.local_executor import LocalToolExecutor
from harness.agent.protocol import RunCommandArgs, ToolProtocol
from harness.agent.test_commands import default_test_command

if TYPE_CHECKING:
    from harness.agent.protocol import (
        EditFileArgs,
        ListFilesArgs,
        ReadFileArgs,
        SearchCodeArgs,
    )

# Default image. Overridable via env or --image; this base image contains the
# seed-task toolchains used by in-container verification (Python, Go, Node).
DEFAULT_IMAGE = os.environ.get("VULCANBENCH_SANDBOX_IMAGE", "vulcanbench/sandbox:base")

#: The Docker CLI binary; override for hosts where it is not on PATH.
DOCKER_BIN = os.environ.get("VULCANBENCH_DOCKER_BIN", "docker")

_CONTAINER_WORKDIR = "/workspace"

# Exit status of a SIGKILLed process (128 + 9): the signature of an OOM kill,
# but also of any other KILL, so it is only classified together with the
# cgroup's oom_kill counter.
_SIGKILL_EXIT = 137

# The docker CLI's own exit status when the daemon (not the contained command)
# fails: "Error response from daemon", "Cannot connect to the Docker daemon".
_DOCKER_CLI_ERROR_EXIT = 125

# How long to wait on the daemon for lifecycle calls that must not hang a run.
_PROBE_TIMEOUT_S = 30.0
_LIFECYCLE_TIMEOUT_S = 60.0

# Writable paths for non-root containers (host UID/GID). Without these, `go test`
# fails trying to create ~/.cache/go-build under `/` when HOME is unset.
_SANDBOX_ENV = {
    "HOME": "/tmp",
    "GOCACHE": "/tmp/go-build",
    "GOPATH": "/tmp/go",
    "CARGO_HOME": "/tmp/cargo",
}


class SandboxError(RuntimeError):
    """Raised when the sandbox container cannot be created or used."""


@dataclass(frozen=True)
class ResourceSpec:
    """Floor/ceiling resource band for the sandbox container.

    Follows the floor/ceiling separation from Anthropic's infrastructure-noise
    methodology: the floor is a guaranteed allocation (Docker soft limits), the
    ceiling is the hard kill threshold. A pinned single value makes transient
    spikes read as task failures; a band absorbs them while keeping pressure.

    Floors are best-effort on Docker: ``mem_floor`` maps to
    ``--memory-reservation`` (enforced only under host memory pressure) and
    ``cpu_floor`` to a ``--cpu-shares`` weight (a relative guarantee under
    contention, not an absolute reservation). Ceilings are hard:
    ``mem_ceiling`` -> ``--memory`` (OOM kill), ``cpu_ceiling`` -> ``--cpus``
    (throttling), ``pids_limit`` -> ``--pids-limit``.
    """

    mem_floor: str | None = None
    mem_ceiling: str = "2g"
    cpu_floor: float | None = None
    cpu_ceiling: float = 2.0
    pids_limit: int = 512

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class DockerResult:
    """Outcome of one ``docker`` CLI invocation."""

    returncode: int
    stdout: bytes
    stderr: bytes


def _run_docker(args: list[str], *, timeout: float | None = None) -> DockerResult:
    """Run ``docker <args>`` and capture both streams.

    Raises ``OSError`` when the binary is missing and
    ``subprocess.TimeoutExpired`` when ``timeout`` elapses; callers decide what
    each means. Tests replace this function to fake the daemon.
    """
    proc = subprocess.run([DOCKER_BIN, *args], capture_output=True, timeout=timeout, check=False)
    return DockerResult(proc.returncode, proc.stdout, proc.stderr)


def _decode(b: Any) -> str:
    """Decode a (possibly None) bytes chunk from a container exec stream."""
    if isinstance(b, (bytes, bytearray)):
        return bytes(b).decode("utf-8", errors="replace")
    return "" if b is None else str(b)


def _probe_daemon() -> DockerResult:
    """``docker version`` against the server: non-zero when the daemon is down."""
    return _run_docker(["version", "--format", "{{.Server.Version}}"], timeout=_PROBE_TIMEOUT_S)


def _docker_available() -> bool:
    """True if a Docker daemon is reachable. Used for ``auto`` mode and tests."""
    try:
        return _probe_daemon().returncode == 0
    except Exception:
        return False


def _is_daemon_error(result: DockerResult) -> bool:
    """A failure of docker itself (daemon gone, bad container) rather than of the
    command it ran: the CLI exits 125 and explains on stderr."""
    if result.returncode != _DOCKER_CLI_ERROR_EXIT:
        return False
    err = _decode(result.stderr)
    return "Error response from daemon" in err or "Cannot connect to the Docker daemon" in err


class DockerToolExecutor(ToolProtocol):
    """Execute tools against a containerized, bind-mounted workspace."""

    def __init__(
        self,
        workspace: Path | str = ".",
        image: str = DEFAULT_IMAGE,
        network: bool = False,
        mem_limit: str = "2g",
        cpus: float = 2.0,
        pids_limit: int = 512,
        default_timeout: int = 120,
        resources: ResourceSpec | None = None,
    ) -> None:
        self.workspace = Path(workspace).resolve()
        self.workspace.mkdir(parents=True, exist_ok=True)
        # File ops run host-side over the shared bind mount.
        self._files = LocalToolExecutor(self.workspace)
        self.image = image
        self.default_timeout = default_timeout
        self._closed = False
        self._container_id: str | None = None
        # ``resources`` wins over the legacy single-value kwargs; the legacy
        # values become the ceiling of a floorless band, preserving behavior.
        self.resources = resources or ResourceSpec(
            mem_ceiling=mem_limit, cpu_ceiling=cpus, pids_limit=pids_limit
        )
        self.oom_kill_count = 0
        self._oom_baseline: int | None = None

        try:
            probe = _probe_daemon()
        except Exception as e:
            raise SandboxError(
                f"could not connect to the Docker daemon ({e}). Is Docker running?"
            ) from e
        if probe.returncode != 0:
            detail = _decode(probe.stderr).strip() or f"docker exited {probe.returncode}"
            raise SandboxError(
                f"could not connect to the Docker daemon ({detail}). Is Docker running?"
            )

        argv = self._run_argv(image, network=network)
        try:
            started = _run_docker(argv)
        except Exception as e:
            raise SandboxError(f"failed to start sandbox container from {image!r}: {e}") from e
        if started.returncode != 0:
            detail = _decode(started.stderr).strip() or f"docker exited {started.returncode}"
            raise SandboxError(f"failed to start sandbox container from {image!r}: {detail}")
        self._container_id = _decode(started.stdout).strip().splitlines()[-1]

    def _run_argv(self, image: str, *, network: bool) -> list[str]:
        """The ``docker run`` argument vector for the long-lived sandbox container."""
        spec = self.resources
        argv = [
            "run",
            "--detach",
            "--workdir",
            _CONTAINER_WORKDIR,
            "--volume",
            f"{self.workspace}:{_CONTAINER_WORKDIR}:rw",
            "--memory",
            spec.mem_ceiling,
            "--cpus",
            str(float(spec.cpu_ceiling)),
            "--pids-limit",
            str(spec.pids_limit),
            "--security-opt",
            "no-new-privileges",
            "--cap-drop",
            "ALL",
        ]
        for key, value in _SANDBOX_ENV.items():
            argv += ["--env", f"{key}={value}"]
        if not network:
            argv += ["--network", "none"]
        # Run the container as the host UID/GID so files written to the bind mount
        # stay owned by the host user, POSIX only. On Windows os.getuid is absent
        # and Docker Desktop maps bind-mount ownership itself, so we omit --user.
        if hasattr(os, "getuid"):
            argv += ["--user", f"{os.getuid()}:{os.getgid()}"]
        if spec.mem_floor is not None:
            argv += ["--memory-reservation", spec.mem_floor]
        if spec.cpu_floor is not None:
            # cpu-shares is a relative weight (1024 == one default-weight
            # container's worth); a floor of N cpus gets N shares of weight.
            argv += ["--cpu-shares", str(max(2, int(spec.cpu_floor * 1024)))]
        argv += [image, "sleep", "infinity"]
        return argv

    @property
    def container_id(self) -> str | None:
        return self._container_id

    # --- file operations: delegate to the host-side executor ------------------
    def list_files(self, args: ListFilesArgs) -> Any:
        return self._files.list_files(args)

    def read_file(self, args: ReadFileArgs) -> Any:
        return self._files.read_file(args)

    def search_code(self, args: SearchCodeArgs) -> Any:
        return self._files.search_code(args)

    def edit_file(self, args: EditFileArgs) -> Any:
        return self._files.edit_file(args)

    def security_scan(self, timeout_s: float | None = None) -> Any:
        # Static analysis reads files (no untrusted execution) -> host-side.
        return self._files.security_scan(timeout_s=timeout_s)

    # --- execution operations: run inside the container -----------------------
    def run_command(self, args: RunCommandArgs) -> dict[str, Any]:
        workdir = _CONTAINER_WORKDIR
        if args.cwd:
            # Keep the agent within the workspace; reuse the local guard.
            resolved = self._files._resolve(args.cwd)
            workdir = f"{_CONTAINER_WORKDIR}/{resolved.relative_to(self.workspace)}"
        timeout = args.timeout or self.default_timeout
        inner = f"cd {shlex.quote(workdir)} && timeout {timeout}s sh -c {shlex.quote(args.cmd)}"
        return self._exec(inner)

    def run_tests(self) -> dict[str, Any]:
        cmd = default_test_command(self.workspace)
        return self.run_command(RunCommandArgs(cmd=cmd))

    def run_lint(self) -> dict[str, Any]:
        return self.run_command(RunCommandArgs(cmd="ruff check . || true"))

    def run_build(self) -> dict[str, Any]:
        return {"ok": True}

    def _docker_exec(self, cmd: list[str]) -> DockerResult:
        """``docker exec`` ``cmd`` in the sandbox; daemon failures are SandboxError."""
        if self._container_id is None:
            raise SandboxError("sandbox container is not running")
        try:
            result = _run_docker(
                ["exec", "--workdir", _CONTAINER_WORKDIR, self._container_id, *cmd]
            )
        except Exception as e:
            raise SandboxError(f"container exec failed: {e}") from e
        if _is_daemon_error(result):
            raise SandboxError(f"container exec failed: {_decode(result.stderr).strip()}")
        return result

    def _exec(self, shell_cmd: str) -> dict[str, Any]:
        result = self._docker_exec(["sh", "-c", shell_cmd])
        payload = {
            "stdout": _decode(result.stdout),
            "stderr": _decode(result.stderr),
            "exit_code": result.returncode,
        }
        if result.returncode == _SIGKILL_EXIT:
            # A 137 is only an OOM if the cgroup's kill counter moved; plain
            # SIGKILLs (e.g. `timeout -s KILL`) must not count as infra noise.
            new_kills = self._new_oom_kills()
            self.oom_kill_count += new_kills
            payload["oom_killed"] = new_kills > 0
        return payload

    # --- OOM accounting -------------------------------------------------------
    def _read_oom_kills(self) -> int:
        """Total OOM kills in the container's cgroup (v2 or v1), else 0."""
        try:
            result = self._docker_exec(
                [
                    "sh",
                    "-c",
                    "cat /sys/fs/cgroup/memory.events "
                    "/sys/fs/cgroup/memory/memory.oom_control 2>/dev/null || true",
                ]
            )
        except Exception:
            return 0
        kills = 0
        for line in _decode(result.stdout).splitlines():
            parts = line.split()
            if len(parts) == 2 and parts[0] == "oom_kill" and parts[1].isdigit():
                kills = max(kills, int(parts[1]))
        return kills

    def _new_oom_kills(self) -> int:
        """OOM kills since the previous check (first call sets the baseline)."""
        current = self._read_oom_kills()
        if self._oom_baseline is None:
            self._oom_baseline = 0
        new = max(0, current - self._oom_baseline)
        self._oom_baseline = current
        return new

    # --- lifecycle ------------------------------------------------------------
    def close(self) -> None:
        """Stop and remove the container. Safe to call more than once."""
        if getattr(self, "_closed", True):
            return
        container_id = getattr(self, "_container_id", None)
        self._closed = True
        if container_id is None:
            return
        with contextlib.suppress(Exception):
            _run_docker(["stop", "--time", "5", container_id], timeout=_LIFECYCLE_TIMEOUT_S)
        with contextlib.suppress(Exception):
            _run_docker(["rm", "--force", container_id], timeout=_LIFECYCLE_TIMEOUT_S)

    def __enter__(self) -> DockerToolExecutor:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def __del__(self) -> None:  # pragma: no cover - best-effort safety net
        with contextlib.suppress(Exception):
            self.close()

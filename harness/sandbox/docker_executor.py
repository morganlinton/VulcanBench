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
"""

from __future__ import annotations

import contextlib
import os
import shlex
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any

import docker

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

_CONTAINER_WORKDIR = "/workspace"

# Exit status of a SIGKILLed process (128 + 9): the signature of an OOM kill,
# but also of any other KILL, so it is only classified together with the
# cgroup's oom_kill counter.
_SIGKILL_EXIT = 137

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

    Floors are best-effort on Docker: ``mem_floor`` maps to ``mem_reservation``
    (enforced only under host memory pressure) and ``cpu_floor`` to a
    ``cpu_shares`` weight (a relative guarantee under contention, not an
    absolute reservation). Ceilings are hard: ``mem_ceiling`` -> ``mem_limit``
    (OOM kill), ``cpu_ceiling`` -> ``nano_cpus`` (throttling), ``pids_limit``.
    """

    mem_floor: str | None = None
    mem_ceiling: str = "2g"
    cpu_floor: float | None = None
    cpu_ceiling: float = 2.0
    pids_limit: int = 512

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def _decode(b: Any) -> str:
    """Decode a (possibly None) bytes chunk from a container exec stream."""
    if isinstance(b, (bytes, bytearray)):
        return bytes(b).decode("utf-8", errors="replace")
    return "" if b is None else str(b)


def _docker_available() -> bool:
    """True if a Docker daemon is reachable. Used for ``auto`` mode and tests."""
    try:
        client = docker.from_env()
        client.ping()
        return True
    except Exception:
        return False


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
        # ``resources`` wins over the legacy single-value kwargs; the legacy
        # values become the ceiling of a floorless band, preserving behavior.
        self.resources = resources or ResourceSpec(
            mem_ceiling=mem_limit, cpu_ceiling=cpus, pids_limit=pids_limit
        )
        self.oom_kill_count = 0
        self._oom_baseline: int | None = None

        try:
            self._client = docker.from_env()
            self._client.ping()
        except Exception as e:
            raise SandboxError(
                f"could not connect to the Docker daemon ({e}). Is Docker running?"
            ) from e

        # Run the container as the host UID/GID so files written to the bind mount
        # stay owned by the host user, POSIX only. On Windows os.getuid is absent
        # and Docker Desktop maps bind-mount ownership itself, so we omit --user.
        run_kwargs: dict[str, Any] = {}
        if hasattr(os, "getuid"):
            run_kwargs["user"] = f"{os.getuid()}:{os.getgid()}"

        spec = self.resources
        if spec.mem_floor is not None:
            run_kwargs["mem_reservation"] = spec.mem_floor
        if spec.cpu_floor is not None:
            # cpu_shares is a relative weight (1024 == one default-weight
            # container's worth); a floor of N cpus gets N shares of weight.
            run_kwargs["cpu_shares"] = max(2, int(spec.cpu_floor * 1024))
        try:
            self._container = self._client.containers.run(
                image,
                command=["sleep", "infinity"],
                detach=True,
                working_dir=_CONTAINER_WORKDIR,
                volumes={str(self.workspace): {"bind": _CONTAINER_WORKDIR, "mode": "rw"}},
                environment=_SANDBOX_ENV,
                network_disabled=not network,
                mem_limit=spec.mem_ceiling,
                nano_cpus=int(spec.cpu_ceiling * 1_000_000_000),
                pids_limit=spec.pids_limit,
                security_opt=["no-new-privileges"],
                cap_drop=["ALL"],
                tty=False,
                auto_remove=False,
                **run_kwargs,
            )
        except Exception as e:
            raise SandboxError(f"failed to start sandbox container from {image!r}: {e}") from e

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

    def _exec(self, shell_cmd: str) -> dict[str, Any]:
        try:
            result = self._container.exec_run(
                ["sh", "-c", shell_cmd], workdir=_CONTAINER_WORKDIR, demux=True
            )
        except Exception as e:
            raise SandboxError(f"container exec failed: {e}") from e
        raw = result.output
        out_b, err_b = raw if isinstance(raw, tuple) else (raw, None)
        payload = {
            "stdout": _decode(out_b),
            "stderr": _decode(err_b),
            "exit_code": result.exit_code,
        }
        if result.exit_code == _SIGKILL_EXIT:
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
            result = self._container.exec_run(
                [
                    "sh",
                    "-c",
                    "cat /sys/fs/cgroup/memory.events "
                    "/sys/fs/cgroup/memory/memory.oom_control 2>/dev/null || true",
                ],
                demux=True,
            )
        except Exception:
            return 0
        out_b = result.output[0] if isinstance(result.output, tuple) else result.output
        kills = 0
        for line in _decode(out_b).splitlines():
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
        container = getattr(self, "_container", None)
        self._closed = True
        if container is None:
            return
        with contextlib.suppress(Exception):
            container.stop(timeout=5)
        with contextlib.suppress(Exception):
            container.remove(force=True)

    def __enter__(self) -> DockerToolExecutor:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def __del__(self) -> None:  # pragma: no cover - best-effort safety net
        with contextlib.suppress(Exception):
            self.close()

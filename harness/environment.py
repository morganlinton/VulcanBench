"""Multi-service docker-compose environments for tasks (CII v2 Phase 2).

A task opts in through metadata:

    "environment": {
      "compose": "compose.yaml",
      "ready": [
        {"service": "redis", "cmd": "redis-cli ping", "timeout_s": 60}
      ],
      "up_timeout_s": 300
    }

``compose`` is resolved relative to the task directory. Before the agent's
wall-clock budget starts, the harness brings the stack up under a unique
per-run project name, polls every ``ready`` probe via ``docker compose exec``,
resolves the *published* host ports, and writes ``.vb_services.json`` into the
workspace (gitignored, so it never appears in ``final.patch``)::

    {
      "project": "vb-a1b2c3",
      "services": {"redis": {"6379": 55123}}
    }

The agent and the hidden tests read that file to reach the services on
``127.0.0.1``. Compose files must publish ports ephemerally (``ports:
["6379"]``, never ``"6379:6379"``) so concurrent runs cannot collide; the
per-run project name isolates networks, containers, and volumes. Teardown
(``down -v --remove-orphans``) always runs, including on exceptions.

Environment tasks currently require ``--sandbox local``: the docker sandbox
runs agents with networking disabled, so in-container agents cannot reach the
service stack. :func:`start_environment` enforces this.
"""

from __future__ import annotations

import contextlib
import json
import subprocess
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any

from harness.sandbox.docker_executor import SandboxError

if TYPE_CHECKING:  # pragma: no cover
    from harness.tasks import Task

MANIFEST_NAME = ".vb_services.json"
DEFAULT_UP_TIMEOUT_S = 300
DEFAULT_READY_TIMEOUT_S = 60
_READY_POLL_INTERVAL_S = 1.0


def environment_spec(metadata: dict[str, Any]) -> dict[str, Any] | None:
    """The task's ``environment`` object, or None when the task declares none."""
    spec = metadata.get("environment")
    return spec if isinstance(spec, dict) and spec.get("compose") else None


class TaskEnvironment:
    """Lifecycle manager for one run's compose stack.

    Use as a context manager, or call :meth:`up` / :meth:`down` explicitly.
    ``down`` is idempotent and safe to call from ``finally`` blocks.
    """

    def __init__(self, compose_file: Path, project: str, spec: dict[str, Any]):
        self.compose_file = compose_file
        self.project = project
        self.spec = spec
        self.ports: dict[str, dict[str, int]] = {}
        self._up = False

    # -- docker compose plumbing -------------------------------------------

    def _compose(
        self, *args: str, timeout: int, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        cmd = [
            "docker",
            "compose",
            "-p",
            self.project,
            "-f",
            str(self.compose_file),
            *args,
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, check=False)
        if check and proc.returncode != 0:
            raise SandboxError(
                f"docker compose {' '.join(args[:2])} failed for project {self.project}: "
                f"{proc.stderr.strip()[:500]}"
            )
        return proc

    # -- lifecycle ---------------------------------------------------------

    def up(self) -> None:
        timeout = int(self.spec.get("up_timeout_s") or DEFAULT_UP_TIMEOUT_S)
        try:
            self._compose("up", "-d", "--wait", timeout=timeout)
        except subprocess.TimeoutExpired as exc:
            self.down()
            raise SandboxError(
                f"environment for project {self.project} did not come up within {timeout}s"
            ) from exc
        except SandboxError:
            self.down()
            raise
        self._up = True
        try:
            self._wait_ready()
            self._resolve_ports()
        except Exception:
            self.down()
            raise

    def _wait_ready(self) -> None:
        for probe in self.spec.get("ready") or []:
            service = str(probe.get("service") or "")
            cmd = str(probe.get("cmd") or "")
            if not service or not cmd:
                raise SandboxError(f"malformed ready probe in environment spec: {probe!r}")
            timeout = int(probe.get("timeout_s") or DEFAULT_READY_TIMEOUT_S)
            deadline = time.monotonic() + timeout
            last_err = ""
            while time.monotonic() < deadline:
                proc = self._compose(
                    "exec", "-T", service, "sh", "-c", cmd, timeout=30, check=False
                )
                if proc.returncode == 0:
                    break
                last_err = (proc.stderr or proc.stdout or "").strip()[:200]
                time.sleep(_READY_POLL_INTERVAL_S)
            else:
                raise SandboxError(
                    f"service {service!r} not ready within {timeout}s "
                    f"(probe {cmd!r}; last output: {last_err})"
                )

    def _resolve_ports(self) -> None:
        proc = self._compose("ps", "--format", "json", timeout=30)
        self.ports = {}
        for raw_line in proc.stdout.splitlines():
            stripped = raw_line.strip()
            if not stripped:
                continue
            try:
                row = json.loads(stripped)
            except ValueError:
                continue
            service = row.get("Service")
            if not service:
                continue
            published: dict[str, int] = {}
            for pub in row.get("Publishers") or []:
                target, host_port = pub.get("TargetPort"), pub.get("PublishedPort")
                if target and host_port:
                    published[str(target)] = int(host_port)
            if published:
                self.ports[service] = published

    def write_manifest(self, workspace: Path) -> Path:
        """Write ``.vb_services.json`` into the workspace and gitignore it."""
        manifest = workspace / MANIFEST_NAME
        manifest.write_text(
            json.dumps({"project": self.project, "services": self.ports}, indent=2) + "\n",
            encoding="utf-8",
        )
        gitignore = workspace / ".gitignore"
        existing = gitignore.read_text(encoding="utf-8") if gitignore.exists() else ""
        if MANIFEST_NAME not in existing:
            joiner = "" if not existing or existing.endswith("\n") else "\n"
            gitignore.write_text(existing + joiner + MANIFEST_NAME + "\n", encoding="utf-8")
        return manifest

    def down(self) -> None:
        with contextlib.suppress(subprocess.TimeoutExpired, OSError):  # best effort
            self._compose("down", "-v", "--remove-orphans", "-t", "10", timeout=120, check=False)
        self._up = False

    def __enter__(self) -> TaskEnvironment:
        self.up()
        return self

    def __exit__(self, *exc: object) -> None:
        self.down()


def start_environment(
    task: Task, run_token: str, workspace: Path, sandbox: str = "local"
) -> TaskEnvironment | None:
    """Bring up the task's environment, if it declares one.

    Returns None for tasks without an ``environment`` block. Raises
    :class:`SandboxError` when the stack cannot come up, when compose is
    unavailable, or when the run uses the network-disabled docker sandbox.
    """
    spec = environment_spec(task.metadata)
    if spec is None:
        return None
    if sandbox == "docker":
        raise SandboxError(
            "this task declares a multi-service environment, which requires "
            "--sandbox local (the docker sandbox runs agents without networking)"
        )
    compose_file = (task.root / str(spec["compose"])).resolve()
    if not compose_file.is_file():
        raise SandboxError(f"environment compose file not found: {compose_file}")
    probe = subprocess.run(
        ["docker", "compose", "version"], capture_output=True, text=True, check=False
    )
    if probe.returncode != 0:
        raise SandboxError("docker compose is not available on this host")
    # Project names must be lowercase alphanumerics/dashes and unique per run.
    project = f"vb-{run_token.lower().replace('_', '-')[-40:]}"
    env = TaskEnvironment(compose_file, project, spec)
    env.up()
    env.write_manifest(workspace)
    return env

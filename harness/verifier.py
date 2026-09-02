"""Declarative task verifier.

A task declares its tests in ``metadata.tests`` as two lists of
``{"name", "cmd"}`` entries; each ``cmd`` runs in the workspace and **exit code
0 means the test passed**:

- ``fail_to_pass``: must fail on the starting repo and pass after the fix, the
  real signal. ``functional`` is the fraction of these that pass.
- ``pass_to_pass``: must keep passing (regression guard). If any of these fail,
  ``functional`` is gated to 0.0 regardless of the fail-to-pass results.

This runs at scoring time, after copying the task's hidden ``tests/`` into the
workspace (so the agent never saw them while solving).

Test commands are dispatched through a ``Runner``: a callable that takes
``(cmd, workspace, timeout)`` and returns an exit code. The default runs on the
host; the agent loop passes a runner that ``exec``s inside the Docker sandbox so
verification happens in the same isolated, reproducible environment as the run.
"""

from __future__ import annotations

import subprocess
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from harness.tasks import Task, install_hidden_tests

DEFAULT_TIMEOUT = 120


@dataclass(frozen=True)
class RunnerOutcome:
    """Captured verifier command result used to distinguish infra failures."""

    exit_code: int
    stdout: str = ""
    stderr: str = ""


class VerifierInfrastructureError(RuntimeError):
    """The verifier environment failed before the task received a verdict."""


# (cmd, workspace, timeout) -> exit code/result (0 == pass). Integer runners
# remain supported for task validation fixtures and third-party integrations.
Runner = Callable[[str, Path, int], int | RunnerOutcome]


def _run_host_captured(cmd: str, workspace: Path, timeout: int) -> RunnerOutcome:
    try:
        proc = subprocess.run(
            cmd,
            shell=True,
            cwd=workspace,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return RunnerOutcome(124, str(exc.stdout or ""), str(exc.stderr or ""))
    return RunnerOutcome(proc.returncode, proc.stdout, proc.stderr)


def host_runner(cmd: str, workspace: Path, timeout: int) -> int:
    """Run a test command on the host, in the workspace; returns its exit code."""
    return _run_host_captured(cmd, workspace, timeout).exit_code


def _infrastructure_reason(cmd: str, outcome: RunnerOutcome) -> str | None:
    """Return a reason when a test runner is missing its required toolchain."""
    output = f"{outcome.stdout}\n{outcome.stderr}".lower()
    if outcome.exit_code == 124:
        return "verifier command timed out"
    if "no module named pytest" in output or "no module named 'pytest'" in output:
        return "pytest is unavailable in the verifier environment"
    missing_commands = ("python", "python3", "pytest", "go", "cargo", "npm", "node")
    if outcome.exit_code in {126, 127} and any(
        cmd.lstrip().startswith(executable) for executable in missing_commands
    ):
        return "verifier toolchain command is unavailable"
    return None



def _normalize_test_cmd(cmd: str) -> str:
    # Neutralize the repo-root pyproject.toml `addopts = --cov=...` leak.
    # pytest auto-discovers the inifile up from the workspace, so running under
    # `runs/<task>-*/workspace` resolves rootdir to the VulcanBench repo root and
    # inherits `--cov=harness --cov-report=... --cov-fail-under=...`. Those flags
    # make pytest error before running any test, so functionally-correct patches
    # are scored 0.0. Appending `-o addopts=` clears the inherited addopts so the
    # gold tests actually run.
    if "pytest" in cmd and "-o addopts" not in cmd: return cmd + " -o addopts="
    return cmd

def _run_group(
    entries: list[dict[str, Any]], workspace: Path, timeout: int, runner: Runner
) -> dict[str, bool]:
    results: dict[str, bool] = {}
    for i, entry in enumerate(entries):
        name = str(entry.get("name") or f"test_{i}")
        cmd = entry.get("cmd")
        if not cmd:
            results[name] = False
            continue
        raw_outcome = runner(_normalize_test_cmd(str(cmd)), workspace, timeout)
        outcome = (
            raw_outcome
            if isinstance(raw_outcome, RunnerOutcome)
            else RunnerOutcome(int(raw_outcome))
        )
        reason = _infrastructure_reason(str(cmd), outcome)
        if reason:
            raise VerifierInfrastructureError(f"{reason}: {name} ({cmd})")
        results[name] = outcome.exit_code == 0
    return results


def run_declarative_verifier(
    task: Task,
    workspace: Path,
    runner: Runner | None = None,
    timeout: int = DEFAULT_TIMEOUT,
) -> dict[str, Any]:
    """Run a task's declarative tests and return a scores payload.

    ``runner`` defaults to running tests on the host; pass a sandbox runner to
    verify inside a container. Either way, hidden tests are copied into the
    workspace first (the workspace is the container's bind mount under Docker).
    """
    runner = runner or _run_host_captured
    install_hidden_tests(task, workspace)
    spec = task.tests_spec or {}
    fail_to_pass = list(spec.get("fail_to_pass") or [])
    pass_to_pass = list(spec.get("pass_to_pass") or [])

    if not fail_to_pass:
        return {"scores": {"functional": 0.0, "error": "no fail_to_pass tests declared"}}

    p2p_results = _run_group(pass_to_pass, workspace, timeout, runner)
    f2p_results = _run_group(fail_to_pass, workspace, timeout, runner)

    p2p_ok = all(p2p_results.values())
    f2p_passing = sum(1 for ok in f2p_results.values() if ok)
    functional = 0.0 if not p2p_ok else round(f2p_passing / len(f2p_results), 4)

    details = []
    if not p2p_ok:
        broke = [n for n, ok in p2p_results.items() if not ok]
        details.append(f"regression: pass_to_pass failing: {broke}")
    details.append(f"fail_to_pass {f2p_passing}/{len(f2p_results)} passing")

    return {
        "scores": {"functional": functional},
        "fail_to_pass": f2p_results,
        "pass_to_pass": p2p_results,
        "pass_to_pass_ok": p2p_ok,
        "details": details,
    }

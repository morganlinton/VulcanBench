"""Re-grade an existing run against the current task definition, at zero API cost.

An agent run is expensive (it calls a model); grading is deterministic and free.
When a task's hidden tests or thresholds change (e.g. a calibration), the old
runs do not need to be re-executed against the model — only re-graded. This
module rebuilds the graded workspace from the task's base snapshot plus the run's
captured ``final.patch`` (the agent's edits), overlays the *current* hidden
tests, and re-runs the verifier in the sandbox. No provider is ever called.

The reconstruction (base + agent patch + current tests) is used rather than the
archived ``workspace/`` directory because that directory still contains the
*old* hidden tests from the original grading; rebuilding from the patch keeps the
regrade faithful to whatever the task's tests say now.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from harness.agent.protocol import RunCommandArgs
from harness.sandbox.docker_executor import DockerToolExecutor
from harness.sandbox.images import resolve_sandbox_image
from harness.tasks import load_task, prepare_workspace, task_hash
from harness.verifier import DEFAULT_TIMEOUT, run_declarative_verifier

DEFAULT_TASKS_BASE = Path("tasks")


def resolve_tasks_root(task_id: str, tasks_base: Path = DEFAULT_TASKS_BASE) -> Path | None:
    """Find the ``tasks/<suite>`` directory that defines ``task_id``.

    Runs record only the task id (and an ephemeral suite name), so on regrade we
    locate the task by scanning the task roots. Returns ``None`` if not found.
    """
    if not tasks_base.is_dir():
        return None
    for root in sorted(p for p in tasks_base.iterdir() if p.is_dir()):
        if (root / task_id / "metadata.json").is_file():
            return root
    return None


def _git(args: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    env_names = ("AUTHOR", "COMMITTER")
    env = {}
    for who in env_names:
        env[f"GIT_{who}_NAME"] = "vulcanbench"
        env[f"GIT_{who}_EMAIL"] = "bot@vulcanbench"
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
        env={**os.environ, **env},
    )


def _apply_patch(workspace: Path, patch: str) -> None:
    if not patch.strip():
        return
    proc = subprocess.run(
        ["git", "apply", "--whitespace=nowarn"],
        cwd=workspace,
        input=patch,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        proc3 = subprocess.run(
            ["git", "apply", "--3way", "--whitespace=nowarn"],
            cwd=workspace,
            input=patch,
            capture_output=True,
            text=True,
            check=False,
        )
        if proc3.returncode != 0:
            raise RuntimeError(f"patch did not apply: {proc.stderr.strip()}")


def _docker_runner(executor: DockerToolExecutor):
    def run(cmd: str, _workspace: Path, timeout: int) -> int:
        try:
            res = executor.run_command(RunCommandArgs(cmd=cmd, timeout=timeout))
        except Exception:
            return 124
        return int(res.get("exit_code", 1))

    return run


def regrade_run(
    run_dir: Path,
    tasks_base: Path = DEFAULT_TASKS_BASE,
    sandbox: str = "docker",
    image: str | None = None,
    write: bool = True,
) -> dict[str, Any]:
    """Re-grade one run directory against the current task definition.

    Returns a record with old vs new functional score and per-test results.
    Writes ``regrade.json`` into ``run_dir`` when ``write`` is set. Never calls a
    model provider.
    """
    summary_path = run_dir / "summary.json"
    if not summary_path.is_file():
        return {"run_dir": str(run_dir), "error": "no summary.json"}
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    task_id = summary.get("task_id")
    old_functional = (summary.get("scores") or {}).get("functional")

    root = resolve_tasks_root(task_id, tasks_base)
    if root is None:
        return {"run_dir": str(run_dir), "task_id": task_id, "error": "task not found in tasks/"}
    task = load_task(task_id, root)

    if task.tests_spec is None:
        return {"run_dir": str(run_dir), "task_id": task_id, "error": "task is not test-graded"}

    patch_path = run_dir / "final.patch"
    if not patch_path.is_file():
        return {"run_dir": str(run_dir), "task_id": task_id, "error": "no final.patch to regrade"}
    patch = patch_path.read_text(encoding="utf-8")

    workspace = run_dir / "regrade_workspace"
    if workspace.exists():
        shutil.rmtree(workspace)
    try:
        prepare_workspace(task, workspace)
        _git(["init", "-q"], workspace)
        _git(["add", "-A"], workspace)
        _git(["commit", "-q", "--allow-empty", "-m", "base"], workspace)
        _apply_patch(workspace, patch)
    except RuntimeError as e:
        return {"run_dir": str(run_dir), "task_id": task_id, "old_functional": old_functional,
                "error": str(e)}

    executor: DockerToolExecutor | None = None
    try:
        if sandbox == "docker":
            resolved_image = resolve_sandbox_image(task, image)
            executor = DockerToolExecutor(workspace, image=resolved_image, network=False)
            runner = _docker_runner(executor)
        else:
            runner = None  # host runner (default in run_declarative_verifier)
        payload = run_declarative_verifier(task, workspace, runner=runner, timeout=DEFAULT_TIMEOUT)
    finally:
        if executor is not None:
            close = getattr(executor, "close", None)
            if callable(close):
                close()

    new_functional = float((payload.get("scores") or {}).get("functional", 0.0))
    record = {
        "run_dir": str(run_dir),
        "run_id": summary.get("run_id"),
        "task_id": task_id,
        "model": summary.get("model"),
        "old_functional": old_functional,
        "new_functional": new_functional,
        "delta": None if old_functional is None else round(new_functional - old_functional, 4),
        "fail_to_pass": payload.get("fail_to_pass"),
        "pass_to_pass": payload.get("pass_to_pass"),
        "old_task_hash": summary.get("task_hash"),
        "new_task_hash": task_hash(task),
        "regraded_at": datetime.now(UTC).isoformat(),
        "sandbox": sandbox,
    }
    if write:
        (run_dir / "regrade.json").write_text(json.dumps(record, indent=2), encoding="utf-8")
    # Clean the throwaway workspace to avoid bloating the run dir.
    shutil.rmtree(workspace, ignore_errors=True)
    return record


def find_run_dirs(target: Path) -> list[Path]:
    """Return run directories under ``target`` (or ``[target]`` if it is one)."""
    if (target / "summary.json").is_file():
        return [target]
    return sorted(p.parent for p in target.glob("**/summary.json"))

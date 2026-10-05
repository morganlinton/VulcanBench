#!/usr/bin/env python3
"""Validate every OCaml pilot check and incomplete-solution control serially."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import asdict
from pathlib import Path
from typing import Any

from harness.agent.local_executor import LocalToolExecutor
from harness.agent.loop import _executor_runner_outcome, _git_init
from harness.sandbox.docker_executor import DockerToolExecutor
from harness.sandbox.images import resolve_sandbox_image
from harness.tasks import load_task, prepare_workspace, run_setup, task_hash
from harness.validate import ValidateOptions, validate_task
from harness.verifier import RunnerOutcome, run_declarative_verifier


def contains_ocaml_compile_error(output: str) -> bool:
    """Distinguish compiler diagnostics from Python assertion failures."""
    plain = re.sub(r"\x1b\[[0-9;]*m", "", output)
    return re.search(r"(?m)^\s*Error(?::|\s*\(warning\b)", plain) is not None


class ExerciseError(RuntimeError):
    """Keep failed setup output instead of losing the infrastructure receipt."""

    def __init__(self, message: str, commands: list[dict[str, Any]]) -> None:
        super().__init__(message)
        self.commands = commands


def exercise(
    root: Path,
    patch: Path | None,
    *,
    initialize_git: bool = False,
    supplemental_commands: tuple[str, ...] = (),
    sandbox: str = "docker",
) -> dict[str, Any]:
    """Return verdicts from a fresh workspace; Docker enforces network isolation."""
    if sandbox not in {"local", "docker"}:
        raise ValueError("sandbox must be local or docker")
    task = load_task(root.name, root.parent)
    with tempfile.TemporaryDirectory(prefix="ocaml-validation-") as temporary:
        workspace = Path(temporary) / "workspace"
        prepare_workspace(task, workspace)
        assert not (workspace / "hidden").exists(), "hidden tests leaked before verification"
        assert not (workspace / "gold_patch.diff").exists(), "reference patch leaked"
        if initialize_git:
            _git_init(workspace)
        if patch is not None:
            subprocess.run(["git", "apply", str(patch.resolve())], cwd=workspace, check=True)
        executor = (
            DockerToolExecutor(workspace, image=resolve_sandbox_image(task))
            if sandbox == "docker"
            else LocalToolExecutor(workspace)
        )
        captured: list[dict[str, Any]] = []
        try:

            def runner(cmd: str, path: Path, timeout: int) -> RunnerOutcome:
                return _executor_runner_outcome(executor, cmd, timeout)

            setup_commands: list[dict[str, Any]] = []

            def preflight(cmd: str, path: Path, timeout: int) -> RunnerOutcome:
                outcome = runner(cmd, path, timeout)
                setup_commands.append({"cmd": cmd, **asdict(outcome)})
                return outcome

            try:
                setup = run_setup(task, workspace, runner=preflight)
            except RuntimeError as exc:
                raise ExerciseError(str(exc), setup_commands) from exc

            def capture(cmd: str, path: Path, timeout: int) -> RunnerOutcome:
                outcome = runner(cmd, path, timeout)
                captured.append({"cmd": cmd, **asdict(outcome)})
                return outcome

            verdict = run_declarative_verifier(
                task, workspace, runner=capture, timeout=task.metadata["test_timeout_s"]
            )
            supplemental = [
                {"cmd": cmd, **asdict(runner(cmd, workspace, task.metadata["test_timeout_s"]))}
                for cmd in supplemental_commands
            ]
            return {
                "setup": setup,
                "setup_commands": setup_commands,
                "verdict": verdict,
                "commands": captured,
                "supplemental_commands": supplemental,
                "task_hash": task_hash(task),
                "sandbox": sandbox,
            }
        finally:
            if isinstance(executor, DockerToolExecutor):
                executor.close()


def relaunch_offline(args: argparse.Namespace) -> int | None:
    """Run the complete validation and its children under macOS network denial."""
    if not args.offline:
        return None
    if args.sandbox != "local":
        raise RuntimeError(
            "--offline is for native macOS; Docker validation already disables network"
        )
    sandbox_exec = shutil.which("sandbox-exec")
    if not sandbox_exec:
        raise RuntimeError("--offline requires the macOS sandbox-exec utility")
    if os.environ.get("VULCANBENCH_OCAML_OFFLINE_CHILD") == "1":
        return None
    command = [
        sandbox_exec,
        "-p",
        "(version 1) (allow default) (deny network*)",
        sys.executable,
        str(Path(__file__).resolve()),
        *sys.argv[1:],
    ]
    return subprocess.run(
        command, env={**os.environ, "VULCANBENCH_OCAML_OFFLINE_CHILD": "1"}, check=False
    ).returncode


def parse_arguments() -> tuple[argparse.ArgumentParser, argparse.Namespace]:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tasks-root", type=Path, default=Path("tasks/ocaml-v1"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--task", action="append", help="Validate selected pilot task ids only")
    parser.add_argument("--sandbox", choices=("docker", "local"), default="docker")
    parser.add_argument(
        "--offline",
        action="store_true",
        help="Deny network for the entire native macOS validation process",
    )
    args = parser.parse_args()
    return parser, args


def main() -> int:
    parser, args = parse_arguments()
    try:
        relaunched = relaunch_offline(args)
    except RuntimeError as exc:
        parser.error(str(exc))
    if relaunched is not None:
        return relaunched
    report: dict[str, Any] = {
        "tasks": [],
        "valid": True,
        "difficulty": "unmeasured",
        "sandbox": args.sandbox,
        "network_isolation_enforced": args.sandbox == "docker" or args.offline,
    }
    manifest = json.loads((args.tasks_root / "suite.json").read_text())
    if args.task:
        unknown = set(args.task) - set(manifest["tasks"])
        if unknown:
            parser.error(f"unknown pilot tasks: {sorted(unknown)}")
    for task_id in manifest["tasks"]:
        if args.task and task_id not in args.task:
            continue
        root = args.tasks_root / task_id
        print(f"Validating {task_id}", flush=True)
        row: dict[str, Any] = {"id": task_id, "base": [], "gold": [], "controls": []}
        try:
            for repetition in range(3):
                base = exercise(root, None, sandbox=args.sandbox)
                row["base"].append(base)
                gold = exercise(root, root / "gold_patch.diff", sandbox=args.sandbox)
                row["gold"].append(gold)
                assert base["verdict"]["pass_to_pass_ok"], "base regression guard failed"
                assert not any(base["verdict"]["fail_to_pass"].values()), "pre-solved check"
                assert gold["verdict"]["scores"]["functional"] == 1, "reference failed"
                print(f"  base/reference repetition {repetition + 1}: PASS", flush=True)
            for patch in sorted((root / "controls").glob("*.diff")):
                control = exercise(root, patch, sandbox=args.sandbox)
                row["controls"].append({"patch": patch.name, **control})
                assert control["verdict"]["pass_to_pass_ok"], "control broke a regression guard"
                assert control["verdict"]["scores"]["functional"] < 1, "control passed"
                for command in control["commands"]:
                    output = command["stdout"] + command["stderr"]
                    assert not contains_ocaml_compile_error(output), "control rejected by compiler"
                print(f"  incomplete-solution control {patch.name}: rejected", flush=True)
            assert row["controls"], "missing incomplete-solution control"
            schema = validate_task(root, ValidateOptions(sandbox=args.sandbox))
            row["standard_validation"] = asdict(schema)
            assert schema.status == "PASS", schema.reasons
            row["valid"] = True
        except (AssertionError, RuntimeError, subprocess.SubprocessError) as exc:
            row["valid"] = False
            row["error"] = str(exc)
            if isinstance(exc, ExerciseError):
                row["failed_setup_commands"] = exc.commands
            report["valid"] = False
            print(f"  FAIL: {exc}", flush=True)
        report["tasks"].append(row)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

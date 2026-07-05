"""Tests for zero-API-cost re-grading of existing runs."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

from harness.regrade import find_run_dirs, regrade_run, resolve_tasks_root
from harness.tasks import load_task, prepare_workspace

pytestmark = pytest.mark.skipif(shutil.which("pytest") is None, reason="pytest not on PATH")


def _make_task(suite: Path, f2p_expected: int = 2) -> None:
    """A tiny Python task under ``suite/fix-f`` whose base code returns 0."""
    task = suite / "fix-f"
    (task / "repo").mkdir(parents=True)
    (task / "tests").mkdir(parents=True)
    (task / "repo" / "m.py").write_text("def f():\n    return 0\n")
    (task / "tests" / "t_f2p.py").write_text(
        f"from m import f\n\ndef test_expected():\n    assert f() == {f2p_expected}\n"
    )
    (task / "tests" / "t_p2p.py").write_text(
        "from m import f\n\ndef test_int():\n    assert isinstance(f(), int)\n"
    )
    (task / "metadata.json").write_text(
        json.dumps(
            {
                "id": "fix-f",
                "category": "bug_fix",
                "languages": ["python"],
                "difficulty": "trivial",
                "created": "2026-07-05",
                "source": "hand-authored",
                "decontamination_notes": "fixture",
                "tests": {
                    "fail_to_pass": [{"name": "exp", "cmd": "python -m pytest t_f2p.py -q"}],
                    "pass_to_pass": [{"name": "int", "cmd": "python -m pytest t_p2p.py -q"}],
                },
            }
        )
    )
    (task / "issue.md").write_text("make f return the expected value")


def _make_run(run_dir: Path, patch: str, old_functional: float) -> None:
    """Fabricate a completed-run directory: a summary + the agent's captured patch."""
    run_dir.mkdir(parents=True)
    (run_dir / "summary.json").write_text(
        json.dumps(
            {
                "run_id": run_dir.name,
                "task_id": "fix-f",
                "model": "mock:test",
                "scores": {"functional": old_functional},
                "task_hash": "old",
            }
        )
    )
    (run_dir / "final.patch").write_text(patch)


def _patch_that_returns(suite: Path, tmp: Path, value: int) -> str:
    """Build a git patch (like a real run's final.patch) that makes f() return ``value``."""
    task = load_task("fix-f", suite)
    ws = prepare_workspace(task, tmp / "patchws")
    env = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
           "GIT_COMMITTER_EMAIL": "t@t"}
    full = {**os.environ, **env}
    subprocess.run(["git", "init", "-q"], cwd=ws, check=True, env=full)
    subprocess.run(["git", "add", "-A"], cwd=ws, check=True, env=full)
    subprocess.run(["git", "commit", "-qm", "base"], cwd=ws, check=True, env=full)
    (ws / "m.py").write_text(f"def f():\n    return {value}\n")
    subprocess.run(["git", "add", "-A"], cwd=ws, check=True, env=full)
    diff = subprocess.run(
        ["git", "diff", "--cached"], cwd=ws, capture_output=True, text=True, check=True, env=full
    )
    shutil.rmtree(ws)
    return diff.stdout


def test_regrade_reconstructs_and_scores_the_patch(tmp_path: Path) -> None:
    suite = tmp_path / "tasks"
    _make_task(suite, f2p_expected=2)
    patch = _patch_that_returns(suite, tmp_path, value=2)  # a correct fix

    run_dir = tmp_path / "runs" / "fix-f-abc123"
    _make_run(run_dir, patch, old_functional=0.0)

    rec = regrade_run(run_dir, tasks_base=tmp_path, sandbox="local", write=True)

    assert rec["old_functional"] == 0.0
    assert rec["new_functional"] == 1.0  # base(0) + patch(2) + tests(==2) -> passes
    assert rec["delta"] == 1.0
    assert rec["fail_to_pass"]  # per-test detail present
    # regrade.json is written back into the run dir, and the throwaway ws is cleaned.
    assert (run_dir / "regrade.json").is_file()
    assert not (run_dir / "regrade_workspace").exists()


def test_regrade_reflects_a_changed_test(tmp_path: Path) -> None:
    """The whole point: changing the task's tests re-scores an old run for free."""
    suite = tmp_path / "tasks"
    _make_task(suite, f2p_expected=2)
    patch = _patch_that_returns(suite, tmp_path, value=2)
    run_dir = tmp_path / "runs" / "fix-f-def456"
    _make_run(run_dir, patch, old_functional=1.0)

    # Raise the bar: the test now demands f() == 3. The old patch (returns 2) fails.
    (suite / "fix-f" / "tests" / "t_f2p.py").write_text(
        "from m import f\n\ndef test_expected():\n    assert f() == 3\n"
    )

    rec = regrade_run(run_dir, tasks_base=tmp_path, sandbox="local", write=False)
    assert rec["old_functional"] == 1.0
    assert rec["new_functional"] == 0.0
    assert rec["delta"] == -1.0


def test_resolve_tasks_root_and_find_run_dirs(tmp_path: Path) -> None:
    suite = tmp_path / "tasks"
    _make_task(suite)
    assert resolve_tasks_root("fix-f", tmp_path) == suite
    assert resolve_tasks_root("nope", tmp_path) is None

    runs = tmp_path / "runs"
    _make_run(runs / "a", "", 0.0)
    _make_run(runs / "b", "", 0.0)
    found = find_run_dirs(runs)
    assert {p.name for p in found} == {"a", "b"}
    # Pointing directly at a single run dir returns just that one.
    assert find_run_dirs(runs / "a") == [runs / "a"]


def test_regrade_missing_patch_is_reported(tmp_path: Path) -> None:
    suite = tmp_path / "tasks"
    _make_task(suite)
    run_dir = tmp_path / "runs" / "fix-f-nopatch"
    run_dir.mkdir(parents=True)
    (run_dir / "summary.json").write_text(
        json.dumps({"run_id": "x", "task_id": "fix-f", "scores": {"functional": 0.0}})
    )
    rec = regrade_run(run_dir, tasks_base=tmp_path, sandbox="local")
    assert "no final.patch" in rec["error"]

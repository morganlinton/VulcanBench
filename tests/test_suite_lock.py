"""Tests for suite locks (tasks/<suite>/suite.lock.json)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from harness import suite_lock


def _task(root: Path, task_id: str, issue: str = "Fix it.") -> None:
    d = root / task_id
    (d / "repo").mkdir(parents=True)
    (d / "repo" / "app.py").write_text("x = 1\n")
    (d / "issue.md").write_text(issue)
    (d / "metadata.json").write_text(json.dumps({"id": task_id, "difficulty": "hard"}))


@pytest.fixture
def base(tmp_path: Path) -> Path:
    suite = tmp_path / "tasks" / "demo-v1"
    _task(suite, "a")
    _task(suite, "b")
    (suite / "suite.json").write_text(json.dumps({"tasks": ["a", "b"], "version": "1.0.0"}))
    return tmp_path / "tasks"


def test_freeze_then_check_passes(base: Path) -> None:
    path = suite_lock.freeze("demo-v1", base)
    lock = json.loads(path.read_text())
    assert lock["version"] == "1.0.0"
    assert set(lock["tasks"]) == {"a", "b"}
    assert suite_lock.locked_suites(base) == ["demo-v1"]
    assert suite_lock.check_suite("demo-v1", base).problems == []
    assert suite_lock.main(["--tasks-base", str(base), "check"]) == 0


def test_cosmetic_metadata_edit_is_allowed(base: Path) -> None:
    suite_lock.freeze("demo-v1", base)
    meta = base / "demo-v1" / "a" / "metadata.json"
    meta.write_text(json.dumps({"id": "a", "difficulty": "medium"}))
    assert suite_lock.check_suite("demo-v1", base).problems == []


def test_scoring_edit_fails(base: Path, capsys: pytest.CaptureFixture[str]) -> None:
    suite_lock.freeze("demo-v1", base)
    (base / "demo-v1" / "a" / "issue.md").write_text("Fix it differently.")
    assert suite_lock.check_suite("demo-v1", base).problems == ["task changed: a"]
    assert suite_lock.main(["--tasks-base", str(base), "check"]) == 1
    assert "FAIL demo-v1" in capsys.readouterr().out


def test_task_set_changes_fail(base: Path) -> None:
    suite_lock.freeze("demo-v1", base)
    _task(base / "demo-v1", "c")
    (base / "demo-v1" / "suite.json").write_text(
        json.dumps({"tasks": ["a", "c"], "version": "1.0.0"})
    )
    assert suite_lock.check_suite("demo-v1", base).problems == [
        "task added: c",
        "task removed: b",
    ]


def test_version_bump_requires_refreeze(base: Path) -> None:
    suite_lock.freeze("demo-v1", base)
    (base / "demo-v1" / "a" / "issue.md").write_text("New prompt.")
    (base / "demo-v1" / "suite.json").write_text(
        json.dumps({"tasks": ["a", "b"], "version": "1.1.0"})
    )
    problems = suite_lock.check_suite("demo-v1", base).problems
    assert len(problems) == 1 and "re-freeze" in problems[0]
    assert suite_lock.main(["--tasks-base", str(base), "freeze", "demo-v1"]) == 0
    assert suite_lock.check_suite("demo-v1", base).problems == []


def test_repo_frozen_suites_match() -> None:
    """The real locks in tasks/ hold for this checkout (CI runs the same check)."""
    assert suite_lock.main(["check"]) == 0

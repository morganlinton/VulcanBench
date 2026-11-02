"""Tests for suite locks (tasks/<suite>/suite.lock.json)."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from harness import suite_lock
from harness.suite import load_suite_dir


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


# --- directory forms, the content digest and verification ---------------------


def test_lock_digest_is_content_not_bytes(base: Path) -> None:
    path = suite_lock.freeze("demo-v1", base)
    lock = json.loads(path.read_text())
    digest = suite_lock.lock_digest(lock)
    assert digest.startswith("sha256:") and len(digest) == len("sha256:") + 64
    # Re-serialised (key order, indentation, date and note) -> same digest.
    reordered = {
        "note": "rewritten",
        "frozen_at": "1999-01-01",
        "tasks": dict(reversed(list(lock["tasks"].items()))),
        "version": lock["version"],
        "suite": lock["suite"],
    }
    assert suite_lock.lock_digest(reordered) == digest
    assert suite_lock.lock_digest(json.loads(json.dumps(lock, indent=None))) == digest
    # A task hash or the version changes it.
    changed = {**lock, "tasks": {**lock["tasks"], "a": "0" * 64}}
    assert suite_lock.lock_digest(changed) != digest
    assert suite_lock.lock_digest({**lock, "version": "9.9.9"}) != digest


def test_freeze_dir_and_check_dir_match_the_in_tree_forms(base: Path) -> None:
    suite_dir = base / "demo-v1"
    path = suite_lock.freeze_dir(suite_dir)
    assert path == suite_dir / suite_lock.LOCK_NAME
    lock = json.loads(path.read_text())
    assert set(lock) == {"suite", "version", "frozen_at", "note", "tasks"}
    assert lock == {
        **suite_lock.compute_lock_dir(suite_dir),
        "frozen_at": lock["frozen_at"],
        "note": lock["note"],
    }
    assert suite_lock.compute_lock_dir(suite_dir) == suite_lock.compute_lock("demo-v1", base)
    assert suite_lock.check_dir(suite_dir).problems == []
    assert suite_lock.check_suite("demo-v1", base).problems == []

    (suite_dir / "b" / "issue.md").write_text("Changed.")
    assert suite_lock.check_dir(suite_dir).problems == ["task changed: b"]
    assert suite_lock.check_suite("demo-v1", base).problems == ["task changed: b"]


def test_check_dir_without_a_lock_is_a_problem_not_a_crash(base: Path) -> None:
    drift = suite_lock.check_dir(base / "demo-v1")
    assert len(drift.problems) == 1 and "vulcanbench suite freeze demo-v1" in drift.problems[0]


def test_verify_lock_reports_missing_drift_and_ok(base: Path) -> None:
    suite_dir = base / "demo-v1"
    missing = suite_lock.verify_lock(suite_dir)
    assert not missing.ok and missing.lock_digest is None and missing.frozen_at is None
    assert missing.version == "1.0.0" and missing.n_tasks == 2
    assert "run `vulcanbench suite freeze demo-v1`" in missing.problems[0]

    suite_lock.freeze_dir(suite_dir)
    ok = suite_lock.verify_lock(suite_dir)
    lock = suite_lock.read_lock(suite_dir)
    assert lock is not None
    assert ok.ok and ok.problems == []
    assert ok.lock_digest == suite_lock.lock_digest(lock)
    assert ok.version == "1.0.0" and ok.frozen_at == lock["frozen_at"] and ok.n_tasks == 2
    assert ok.as_dict()["ok"] is True

    (suite_dir / "a" / "repo" / "app.py").write_text("x = 2\n")
    drift = suite_lock.verify_lock(suite_dir)
    assert drift.problems == ["task changed: a"]
    assert drift.lock_digest == ok.lock_digest  # the lock on disk did not move


def test_verify_lock_names_a_listed_task_with_no_directory(base: Path) -> None:
    suite_dir = base / "demo-v1"
    suite_lock.freeze_dir(suite_dir)
    (suite_dir / "suite.json").write_text(
        json.dumps({"tasks": ["a", "b", "ghost"], "version": "1.0.0"})
    )
    problems = suite_lock.verify_lock(suite_dir).problems
    assert len(problems) == 1 and problems[0].startswith("task missing: ghost")
    with pytest.raises(FileNotFoundError, match="task missing: ghost"):
        suite_lock.freeze_dir(suite_dir)


def test_require_locked_raises_with_every_problem(base: Path) -> None:
    suite_dir = base / "demo-v1"
    suite = load_suite_dir(suite_dir, external=True)
    with pytest.raises(suite_lock.SuiteLockError, match="not frozen"):
        suite_lock.require_locked(suite)
    suite_lock.freeze_dir(suite_dir)
    assert suite_lock.require_locked(suite).ok
    (suite_dir / "a" / "issue.md").write_text("Drifted.")
    (suite_dir / "b" / "issue.md").write_text("Drifted too.")
    with pytest.raises(suite_lock.SuiteLockError) as info:
        suite_lock.require_locked(suite)
    assert "task changed: a" in str(info.value) and "task changed: b" in str(info.value)
    assert "vulcanbench suite freeze demo-v1" in str(info.value)


def test_read_lock(base: Path, tmp_path: Path) -> None:
    assert suite_lock.read_lock(base / "demo-v1") is None
    suite_lock.freeze_dir(base / "demo-v1")
    lock = suite_lock.read_lock(base / "demo-v1")
    assert lock is not None and lock["suite"] == "demo-v1"
    (tmp_path / suite_lock.LOCK_NAME).write_text("[]")
    assert suite_lock.read_lock(tmp_path) is None

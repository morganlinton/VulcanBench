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


def _harbor_task(root: Path, task_id: str, status: str = "candidate") -> Path:
    d = root / task_id
    for sub in ("environment", "tests", "solution"):
        (d / sub).mkdir(parents=True)
    (d / "environment" / "Dockerfile").write_text("FROM scratch\n")
    (d / "tests" / "test.sh").write_text("exit 0\n")
    (d / "solution" / "solve.sh").write_text("true\n")
    (d / "instruction.md").write_text("Fix the three bugs.\n")
    (d / "task.toml").write_text(
        '[task]\nversion = "0.1.0"\n\n'
        f'[metadata]\nvulcanbench_status = "{status}"\n'
        'vulcanbench_determinism_checklist = "py-v1"\n\n'
        "[agent]\ntimeout_sec = 10800.0\n"
    )
    (d / "DESIGN.md").write_text("notes\n")
    return d


@pytest.fixture
def harbor_base(tmp_path: Path) -> tuple[Path, Path]:
    suite = tmp_path / "tasks" / "fv-v5"
    _harbor_task(suite, "pub")
    private = tmp_path / "private" / "tasks"
    _harbor_task(private / "fv-v5", "hidden")
    (suite / "suite.json").write_text(
        json.dumps(
            {
                "tasks": ["pub"],
                "version": "1.0.0",
                "private_tasks": {"hidden": {"repo": "o/private", "commit": "abc"}},
                "pins": {"effort": "medium"},
            }
        )
    )
    return tmp_path / "tasks", private


def test_harbor_freeze_records_meta_private_and_pins(harbor_base: tuple[Path, Path]) -> None:
    base, private = harbor_base
    lock = json.loads(suite_lock.freeze("fv-v5", base, private).read_text())
    assert set(lock["tasks"]) == {"pub"}
    assert lock["task_meta"]["pub"] == {"version": "0.1.0", "checklist": "py-v1"}
    hidden = lock["private_tasks"]["hidden"]
    assert hidden["repo"] == "o/private" and hidden["commit"] == "abc"
    assert hidden["hash"] == suite_lock.harbor_task_hash(private / "fv-v5" / "hidden")
    assert lock["pins"] == {"effort": "medium"}
    assert suite_lock.check_suite("fv-v5", base, private).problems == []


def test_harbor_metadata_and_notes_are_cosmetic(harbor_base: tuple[Path, Path]) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    pub = base / "fv-v5" / "pub"
    toml = (pub / "task.toml").read_text().replace('"candidate"', '"admitted"')
    (pub / "task.toml").write_text(toml)
    (pub / "DESIGN.md").write_text("gate result\n")
    assert suite_lock.check_suite("fv-v5", base, private).problems == []


@pytest.mark.parametrize(
    ("rel", "text", "problem"),
    [
        ("instruction.md", "Fix two bugs.\n", "task changed: pub"),
        ("tests/test.sh", "exit 1\n", "task changed: pub"),
        ("solution/solve.sh", "false\n", "task changed: pub"),
        ("environment/Dockerfile", "FROM busybox\n", "task changed: pub"),
    ],
)
def test_harbor_scoring_edits_fail(
    harbor_base: tuple[Path, Path], rel: str, text: str, problem: str
) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    (base / "fv-v5" / "pub" / rel).write_text(text)
    assert suite_lock.check_suite("fv-v5", base, private).problems == [problem]


def test_harbor_timeout_and_checklist_edits_fail(harbor_base: tuple[Path, Path]) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    toml_path = base / "fv-v5" / "pub" / "task.toml"
    original = toml_path.read_text()
    toml_path.write_text(original.replace("10800.0", "3600.0"))
    assert suite_lock.check_suite("fv-v5", base, private).problems == ["task changed: pub"]
    toml_path.write_text(original.replace('"py-v1"', '"py-v2"'))
    assert suite_lock.check_suite("fv-v5", base, private).problems == [
        "task version or checklist changed: pub"
    ]


def test_private_task_unchecked_without_private_root(
    harbor_base: tuple[Path, Path], capsys: pytest.CaptureFixture[str]
) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    drift = suite_lock.check_suite("fv-v5", base)
    assert drift.problems == [] and drift.unchecked == ["hidden"]
    assert suite_lock.main(["--tasks-base", str(base), "check"]) == 0
    assert "not present here: hidden" in capsys.readouterr().out


def test_private_task_edit_and_source_change_fail(harbor_base: tuple[Path, Path]) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    (private / "fv-v5" / "hidden" / "tests" / "test.sh").write_text("exit 1\n")
    assert suite_lock.check_suite("fv-v5", base, private).problems == [
        "private task changed: hidden"
    ]
    manifest = base / "fv-v5" / "suite.json"
    data = json.loads(manifest.read_text())
    data["private_tasks"]["hidden"]["commit"] = "def"
    manifest.write_text(json.dumps(data))
    assert suite_lock.check_suite("fv-v5", base).problems == ["private task source changed: hidden"]


def test_freeze_refuses_without_private_task(harbor_base: tuple[Path, Path]) -> None:
    base, _ = harbor_base
    with pytest.raises(FileNotFoundError, match="--private-root"):
        suite_lock.freeze("fv-v5", base)


def test_private_root_from_env(
    harbor_base: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    base, private = harbor_base
    monkeypatch.setenv(suite_lock.PRIVATE_ENV, str(private))
    assert suite_lock.main(["--tasks-base", str(base), "freeze", "fv-v5"]) == 0
    assert suite_lock.check_suite("fv-v5", base, suite_lock.private_root_from_env()).unchecked == []


def test_pins_change_fails(harbor_base: tuple[Path, Path]) -> None:
    base, private = harbor_base
    suite_lock.freeze("fv-v5", base, private)
    manifest = base / "fv-v5" / "suite.json"
    data = json.loads(manifest.read_text())
    data["pins"]["effort"] = "high"
    manifest.write_text(json.dumps(data))
    assert suite_lock.check_suite("fv-v5", base, private).problems == ["pins changed in suite.json"]

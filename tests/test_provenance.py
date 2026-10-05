"""Tests for source provenance and the dirty-checkout guard on suite runs."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest
from typer.testing import CliRunner

from harness import provenance
from harness.cli import app


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True)


def _make_repo(root: Path) -> Path:
    root.mkdir()
    _git(root, "init", "-q", "-b", "main")
    _git(root, "config", "user.email", "t@example.com")
    _git(root, "config", "user.name", "t")
    (root / "harness").mkdir()
    (root / "harness" / "x.py").write_text("x = 1\n")
    (root / "docs").mkdir()
    (root / "docs" / "notes.md").write_text("hi\n")
    _git(root, "add", ".")
    _git(root, "commit", "-q", "-m", "init")
    _git(root, "tag", "bench/2026-10-05-test")
    return root


@pytest.fixture
def repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    root = _make_repo(tmp_path / "repo")
    monkeypatch.setattr(provenance, "_HARNESS_DIR", root / "harness")
    monkeypatch.delenv(provenance.ALLOW_DIRTY_ENV, raising=False)
    provenance.clear_cache()
    yield root
    provenance.clear_cache()


def test_clean_checkout_records_commit_and_tag(repo: Path) -> None:
    prov = provenance.source_provenance(repo)
    assert prov.harness is not None
    assert prov.tasks is None  # same checkout: not repeated
    assert prov.harness.describe == "bench/2026-10-05-test"
    assert prov.harness.commit and len(prov.harness.commit) == 40
    assert not prov.dirty
    summary = prov.as_summary()
    assert summary["dirty"] is False
    assert summary["allow_dirty"] is False
    provenance.check_clean_for_suite(cwd=repo)  # does not raise


def test_edits_outside_scoring_paths_do_not_count(repo: Path) -> None:
    (repo / "docs" / "notes.md").write_text("changed\n")
    (repo / "docs" / "new.md").write_text("new\n")
    assert not provenance.source_provenance(repo).dirty


def test_scoring_edit_is_dirty_and_refused(repo: Path) -> None:
    (repo / "harness" / "x.py").write_text("x = 2\n")
    (repo / "tasks").mkdir()
    (repo / "tasks" / "new.json").write_text("{}\n")
    prov = provenance.source_provenance(repo)
    assert prov.dirty
    assert prov.harness is not None
    assert set(prov.harness.dirty_paths) == {"harness/x.py", "tasks/"}
    assert prov.harness.describe == "bench/2026-10-05-test-dirty"
    with pytest.raises(provenance.DirtyTreeError, match=r"harness/x\.py"):
        provenance.check_clean_for_suite(cwd=repo)
    provenance.check_clean_for_suite(allow_dirty=True, cwd=repo)


def test_env_override_allows_and_is_recorded(repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    (repo / "harness" / "x.py").write_text("x = 3\n")
    monkeypatch.setenv(provenance.ALLOW_DIRTY_ENV, "1")
    provenance.check_clean_for_suite(cwd=repo)
    assert provenance.source_provenance(repo).as_summary()["allow_dirty"] is True


def test_separate_task_checkout_is_recorded(repo: Path, tmp_path: Path) -> None:
    other = _make_repo(tmp_path / "suite-repo")
    (other / "harness" / "x.py").write_text("dirty\n")
    prov = provenance.source_provenance(other)
    assert prov.tasks is not None
    assert prov.tasks.root == str(other.resolve())
    assert prov.dirty
    assert prov.dirty_paths() == [f"{other.resolve()}/harness/x.py"]


def test_not_a_git_checkout(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    plain = tmp_path / "plain"
    plain.mkdir()
    monkeypatch.setattr(provenance, "_HARNESS_DIR", plain)
    monkeypatch.delenv(provenance.ALLOW_DIRTY_ENV, raising=False)
    provenance.clear_cache()
    try:
        prov = provenance.source_provenance(plain)
        assert prov.harness is None and prov.tasks is None
        assert prov.as_summary()["harness"] is None
        provenance.check_clean_for_suite(cwd=plain)  # installed package: no refusal
    finally:
        provenance.clear_cache()


def test_cli_suite_run_refuses_dirty_checkout(repo: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    (repo / "harness" / "x.py").write_text("x = 4\n")
    monkeypatch.chdir(repo)
    called = []
    monkeypatch.setattr("harness.cli.run_suite", lambda *a, **k: called.append(1))
    result = CliRunner().invoke(app, ["run", "--suite", "v1", "--model", "openai:gpt-4o-mini"])
    assert result.exit_code == 1
    assert "refusing to start a suite run" in result.output
    assert not called

    sweep = CliRunner().invoke(
        app, ["effort-sweep", "--suite", "v1", "--model", "openai:gpt-4o-mini"]
    )
    assert sweep.exit_code == 1
    assert "refusing to start a suite run" in sweep.output

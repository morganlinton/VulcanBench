"""Tests for scripts/check_pr_scope.py (the PR hygiene workflow)."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from scripts import check_pr_scope as scope
from scripts.check_pr_scope import Change


def test_classify() -> None:
    assert scope.classify("harness/cli.py") == "engine"
    assert scope.classify("tasks/coding-intelligence-index-v4/suite.lock.json") == "engine"
    assert scope.classify("vulcanbench.toml") == "engine"
    assert scope.classify("docs/results/x/card.png") == "results"
    assert scope.classify("traces/run/README.md") == "results"
    assert scope.classify("scripts/cii-v4-board/make_x_card.py") == "neutral"
    assert scope.classify("docs/DECISIONS.md") == "neutral"


def test_mixed_scope_fails_unless_labelled() -> None:
    changes = [Change("A", "harness/maintenance_review_v399.py"), Change("A", "docs/results/a.png")]
    assert len(scope.scope_problems(changes, set())) == 1
    assert scope.scope_problems(changes, {"mixed-scope"}) == []


def test_single_scope_with_neutral_files_passes() -> None:
    engine = [Change("M", "harness/cli.py"), Change("M", "tests/test_cli.py")]
    results = [Change("A", "docs/results/a.png"), Change("A", "scripts/make_card.py")]
    assert scope.scope_problems(engine, set()) == []
    assert scope.scope_problems(results, set()) == []


def test_new_raw_traces_fail_but_readmes_and_deletions_pass() -> None:
    assert scope.trace_problems([Change("A", "traces/x/README.md")]) == []
    assert scope.trace_problems([Change("D", "traces/x/session.jsonl.gz")]) == []
    assert len(scope.trace_problems([Change("A", "traces/x/session.jsonl.gz")])) == 1


def test_large_blobs_fail_outside_tasks() -> None:
    big = scope.MAX_BLOB_BYTES + 1
    changes = [
        Change("A", "docs/results/huge.json"),
        Change("A", "tasks/v9/t/repo/vendor.bin"),
        Change("D", "docs/old.bin"),
    ]
    problems = scope.size_problems(changes, {"docs/results/huge.json": big})
    assert len(problems) == 1 and "huge.json" in problems[0]


def test_main_against_a_real_repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    def git(*args: str) -> None:
        subprocess.run(["git", *args], cwd=tmp_path, check=True, capture_output=True)

    git("init", "-q", "-b", "main")
    git("config", "user.email", "t@example.com")
    git("config", "user.name", "t")
    (tmp_path / "README.md").write_text("x\n")
    git("add", ".")
    git("commit", "-q", "-m", "base")
    git("checkout", "-q", "-b", "feature")
    (tmp_path / "harness").mkdir()
    (tmp_path / "harness" / "a.py").write_text("a = 1\n")
    git("add", ".")
    git("commit", "-q", "-m", "engine")
    monkeypatch.chdir(tmp_path)
    assert scope.main(["--base", "main", "--head", "feature"]) == 0

    (tmp_path / "docs" / "results").mkdir(parents=True)
    (tmp_path / "docs" / "results" / "r.json").write_text("{}\n")
    git("add", ".")
    git("commit", "-q", "-m", "results too")
    assert scope.main(["--base", "main", "--head", "feature"]) == 1
    assert scope.main(["--base", "main", "--head", "feature", "--labels", "mixed-scope"]) == 0

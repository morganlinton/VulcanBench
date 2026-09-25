"""Tests for the security metric."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from harness.evaluator import security
from harness.evaluator.security import assess_security, score_from_counts


def test_score_from_counts_monotonic() -> None:
    assert score_from_counts(0, 0, 0) == 1.0
    assert score_from_counts(0, 0, 1) < 1.0
    assert score_from_counts(1, 0, 0) < score_from_counts(0, 1, 0)
    assert score_from_counts(10, 0, 0) == 0.0  # clamped, never negative


@pytest.mark.skipif(shutil.which("bandit") is None, reason="bandit not installed")
def test_security_flags_shell_injection(tmp_path: Path) -> None:
    (tmp_path / "safe.py").write_text("def add(a, b):\n    return a + b\n")
    (tmp_path / "danger.py").write_text(
        "import subprocess\ndef run(cmd):\n    subprocess.call(cmd, shell=True)\n"
    )
    safe = assess_security(tmp_path, ["safe.py"])
    danger = assess_security(tmp_path, ["danger.py"])
    assert safe.score == 1.0
    assert danger.score is not None and danger.score < 1.0
    assert danger.details["languages"]["python"]["high"] >= 1


def test_security_no_source_files(tmp_path: Path) -> None:
    (tmp_path / "readme.md").write_text("# hi")
    result = assess_security(tmp_path, ["readme.md"])
    assert result.score is None
    assert "reason" in result.details


def test_security_python_missing_bandit(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    (tmp_path / "a.py").write_text("x = 1\n")
    monkeypatch.setattr(security.shutil, "which", lambda name: None)
    result = assess_security(tmp_path, ["a.py"])
    assert result.details["languages"]["python"]["reason"] == "bandit not on PATH"


def test_asserts_in_test_files_are_not_weaknesses(tmp_path: Path) -> None:
    """Bandit's B101 (assert) is skipped in test code and counted elsewhere (owner decision 2026-09-18)."""
    if shutil.which("bandit") is None:
        pytest.skip("bandit not on PATH")
    (tmp_path / "pkg").mkdir()
    (tmp_path / "pkg" / "core.py").write_text("def add(a, b):\n    return a + b\n")
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_core.py").write_text(
        "from pkg.core import add\n\n\ndef test_add():\n    assert add(1, 2) == 3\n    assert add(0, 0) == 0\n"
    )
    (tmp_path / "check.py").write_text("def check(x):\n    assert x > 0\n    return x\n")
    with_tests = assess_security(tmp_path, ["pkg/core.py", "tests/test_core.py"])
    python = with_tests.details["languages"]["python"]
    assert with_tests.score == 1.0
    assert python["low"] == 0
    assert python["skipped_test_asserts"] == 2
    in_source = assess_security(tmp_path, ["check.py"])
    assert in_source.details["languages"]["python"]["low"] == 1
    assert in_source.score is not None and in_source.score < 1.0

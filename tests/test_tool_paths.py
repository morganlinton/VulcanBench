"""Host scoring tools are found beside the running interpreter when PATH lacks them."""

from __future__ import annotations

import stat
import sys
from pathlib import Path

import pytest

from harness.tool_paths import resolve_tool


def _executable(path: Path) -> Path:
    path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return path


def test_resolve_tool_prefers_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    on_path = tmp_path / "path-bin"
    venv_bin = tmp_path / "venv-bin"
    on_path.mkdir()
    venv_bin.mkdir()
    expected = _executable(on_path / "ruff")
    _executable(venv_bin / "ruff")
    monkeypatch.setenv("PATH", str(on_path))
    monkeypatch.setattr(sys, "executable", str(venv_bin / "python"))
    assert resolve_tool("ruff") == str(expected)


def test_resolve_tool_falls_back_to_interpreter_dir(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    venv_bin = tmp_path / "venv-bin"
    venv_bin.mkdir()
    expected = _executable(venv_bin / "bandit")
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    monkeypatch.setattr(sys, "executable", str(venv_bin / "python"))
    assert resolve_tool("bandit") == str(expected)
    assert resolve_tool("no-such-tool") is None

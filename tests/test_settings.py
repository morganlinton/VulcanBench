"""The settings file blocks effort levels at every run entry point."""

from __future__ import annotations

from pathlib import Path

import pytest
from typer.testing import CliRunner

from harness import settings
from harness.agent.loop import run_agent
from harness.cli import app
from harness.settings import (
    BlockedEffortError,
    blocked_efforts,
    check_effort_allowed,
    load_settings,
)


def test_repository_settings_block_ultra() -> None:
    assert settings.SETTINGS_PATH.name == "vulcanbench.toml"
    assert "ultra" in blocked_efforts()
    with pytest.raises(BlockedEffortError):
        check_effort_allowed("ultra")
    with pytest.raises(BlockedEffortError):
        check_effort_allowed(" Ultra ")
    for level in ("minimal", "low", "medium", "high", "extra-high", "max", None):
        check_effort_allowed(level)


def test_missing_settings_file_means_no_restrictions(tmp_path: Path) -> None:
    load_settings.cache_clear()
    try:
        assert blocked_efforts(tmp_path / "absent.toml") == frozenset()
        check_effort_allowed("ultra", tmp_path / "absent.toml")
    finally:
        load_settings.cache_clear()


def test_malformed_block_list_is_rejected(tmp_path: Path) -> None:
    path = tmp_path / "vulcanbench.toml"
    path.write_text('[effort]\nblocked = "ultra"\n')
    load_settings.cache_clear()
    try:
        with pytest.raises(ValueError, match="list of strings"):
            blocked_efforts(path)
    finally:
        load_settings.cache_clear()


def test_run_agent_refuses_blocked_effort_before_loading_anything(tmp_path: Path) -> None:
    with pytest.raises(BlockedEffortError):
        run_agent("no-such-task", "mock:model", output_dir=tmp_path, effort="ultra")
    assert not any(tmp_path.iterdir())


def test_cli_run_and_sweep_refuse_blocked_effort(tmp_path: Path) -> None:
    runner = CliRunner()
    result = runner.invoke(
        app,
        ["run", "--task", "x", "--model", "mock:model", "--effort", "ultra", "-o", str(tmp_path)],
    )
    assert result.exit_code == 1 and "blocked by vulcanbench.toml" in result.output
    result = runner.invoke(
        app,
        [
            "effort-sweep",
            "--suite",
            "v1",
            "--model",
            "mock:model",
            "--efforts",
            "low,ultra",
            "-o",
            str(tmp_path),
        ],
    )
    assert result.exit_code == 1 and "blocked by vulcanbench.toml" in result.output

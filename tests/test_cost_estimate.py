"""Tests for pre-run cost estimation."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from typer.testing import CliRunner

from harness.cli import app
from harness.cost_estimate import estimate_plan

runner = CliRunner()


def _write_summary(runs_dir: Path, run_id: str, payload: dict) -> None:
    d = runs_dir / run_id
    d.mkdir(parents=True)
    (d / "summary.json").write_text(json.dumps(payload), encoding="utf-8")


def test_estimate_exact_history(tmp_path: Path) -> None:
    runs = tmp_path / "runs"
    _write_summary(
        runs,
        "a",
        {
            "task_id": "hello-world",
            "model": "openai:gpt-5.5",
            "cost_usd": 0.02,
            "manifest": {"task": {"repo_scale": "micro"}},
        },
    )
    plan = estimate_plan(
        models=["openai:gpt-5.5"],
        task_ids=["hello-world"],
        judges=False,
        runs_dir=runs,
    )
    assert plan.mid_usd == 0.02
    assert plan.models[0].confidence == "high"


def test_estimate_scales_across_models(tmp_path: Path) -> None:
    runs = tmp_path / "runs"
    _write_summary(
        runs,
        "a",
        {
            "task_id": "hello-world",
            "model": "zai:glm-5.2",
            "cost_usd": 0.01,
            "manifest": {"task": {"repo_scale": "micro"}},
        },
    )
    plan = estimate_plan(
        models=["openai:gpt-5.5"],
        task_ids=["hello-world"],
        judges=False,
        runs_dir=runs,
    )
    assert plan.mid_usd > 0
    assert plan.models[0].per_task[0].source == "task_scaled"


def test_estimate_judges_multiplier(tmp_path: Path) -> None:
    runs = tmp_path / "runs"
    _write_summary(
        runs,
        "a",
        {
            "task_id": "hello-world",
            "model": "openai:gpt-5.5",
            "cost_usd": 0.03,
            "manifest": {"task": {"repo_scale": "micro"}},
        },
    )
    off = estimate_plan(
        models=["openai:gpt-5.5"],
        task_ids=["hello-world"],
        judges=False,
        runs_dir=runs,
    )
    on = estimate_plan(
        models=["openai:gpt-5.5"],
        task_ids=["hello-world"],
        judges=True,
        runs_dir=runs,
    )
    assert on.mid_usd == pytest.approx(off.mid_usd * 3)


def test_cli_estimate_command() -> None:
    result = runner.invoke(
        app,
        [
            "estimate",
            "--task",
            "hello-world",
            "--model",
            "openai:gpt-5.5",
            "--no-judges",
        ],
    )
    assert result.exit_code == 0
    assert "Cost estimate" in result.output
    assert "OPENAI_API_KEY" in result.output


def test_run_dry_run_includes_estimate() -> None:
    result = runner.invoke(
        app,
        [
            "run",
            "--task",
            "hello-world",
            "--model",
            "openai:gpt-5.5",
            "--no-judges",
            "--dry-run",
        ],
    )
    assert result.exit_code == 0
    assert "dry-run" in result.output
    assert "Cost estimate" in result.output

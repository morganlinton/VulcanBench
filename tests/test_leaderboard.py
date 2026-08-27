"""Tests for leaderboard aggregation."""

from __future__ import annotations

import json
from pathlib import Path

from harness.leaderboard import scan_leaderboard


def _write_run(runs: Path, run_id: str, total: float) -> None:
    d = runs / run_id
    d.mkdir(parents=True)
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": run_id,
                "task_id": "hello-world",
                "model": "mock:synthetic",
                "steps": 5,
                "scores": {"functional": 1.0, "total": total},
            }
        )
    )


def test_scan_empty(tmp_path: Path) -> None:
    assert scan_leaderboard(tmp_path / "nope") == []


def test_scan_collects_runs(tmp_path: Path) -> None:
    _write_run(tmp_path, "r1", 0.9)
    _write_run(tmp_path, "r2", 0.8)
    rows = scan_leaderboard(tmp_path)
    ids = {r["run_id"] for r in rows}
    assert ids == {"r1", "r2"}
    assert all(r["functional"] == 1.0 for r in rows)


def test_scan_projects_effort_and_complexity(tmp_path: Path) -> None:
    d = tmp_path / "r1"
    d.mkdir(parents=True)
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": "r1",
                "task_id": "hello-world",
                "model": "mock:synthetic",
                "effort": {
                    "requested": "low",
                    "provider": "mock",
                    "provider_value": None,
                    "supported": False,
                },
                "experiment_id": "experiment-1",
                "manifest": {
                    "task": {
                        "repo_scale": "micro",
                        "task_complexity": "localized",
                        "languages": ["python"],
                        "difficulty": "trivial",
                    }
                },
                "scores": {"functional": 1.0, "total": 0.9},
            }
        )
    )
    row = scan_leaderboard(tmp_path)[0]
    assert row["effort_requested"] == "low"
    assert row["experiment_id"] == "experiment-1"
    assert row["task_complexity"] == "localized"
    assert row["languages"] == ["python"]
    assert row["track"] == "api"
    assert row["execution_harness"] == "vulcan"


def test_scan_projects_subscription_harness_and_economics(tmp_path: Path) -> None:
    d = tmp_path / "subscription-run"
    d.mkdir()
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": "subscription-run",
                "task_id": "hello-world",
                "model": "codex:gpt-5.6-sol",
                "cli_agent": {
                    "harness": "codex",
                    "harness_version": "codex-cli 1.0",
                    "reported_model": None,
                },
                "economics": {
                    "billing_mode": "subscription-included",
                    "cost_basis": "subscription-plus-api-equivalent",
                    "marginal_cash_usd": None,
                    "api_equivalent_cost_usd": 0.12,
                    "plan_name": "plus",
                },
                "scores": {"functional": 1.0, "total": 1.0},
            }
        )
    )
    row = scan_leaderboard(tmp_path)[0]
    assert row["track"] == "subscription"
    assert row["execution_harness"] == "codex"
    assert row["api_equivalent_cost_usd"] == 0.12
    assert row["marginal_cash_usd"] is None


def test_pi_cli_harness_stays_on_api_track(tmp_path: Path) -> None:
    d = tmp_path / "pi-api"
    d.mkdir()
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": "pi-api",
                "task_id": "hello-world",
                "model": "pi:meta:muse-spark-1.2",
                "cli_agent": {
                    "harness": "pi",
                    "billing": "api",
                    "harness_version": "pi 0.52.0",
                },
                "economics": {
                    "billing_mode": "api-metered",
                    "cost_basis": "metered-api-pricing",
                    "marginal_cash_usd": 0.36,
                    "api_equivalent_cost_usd": 0.36,
                },
                "scores": {"functional": 1.0, "total": 1.0},
            }
        )
    )
    row = scan_leaderboard(tmp_path)[0]
    assert row["track"] == "api"
    assert row["execution_harness"] == "pi"
    assert row["marginal_cash_usd"] == 0.36


def test_historical_api_cost_is_used_as_metered_cash(tmp_path: Path) -> None:
    d = tmp_path / "historical-api"
    d.mkdir()
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": "historical-api",
                "task_id": "hello-world",
                "model": "openai:gpt-test",
                "cost_usd": 0.25,
                "scores": {"functional": 1.0, "total": 1.0},
            }
        )
    )
    row = scan_leaderboard(tmp_path)[0]
    assert row["marginal_cash_usd"] == 0.25
    assert row["api_equivalent_cost_usd"] == 0.25


def test_scan_ignores_dirs_without_summary(tmp_path: Path) -> None:
    (tmp_path / "incomplete").mkdir()
    _write_run(tmp_path, "good", 0.5)
    rows = scan_leaderboard(tmp_path)
    assert [r["run_id"] for r in rows] == ["good"]

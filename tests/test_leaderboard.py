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


def test_scan_ignores_dirs_without_summary(tmp_path: Path) -> None:
    (tmp_path / "incomplete").mkdir()
    _write_run(tmp_path, "good", 0.5)
    rows = scan_leaderboard(tmp_path)
    assert [r["run_id"] for r in rows] == ["good"]

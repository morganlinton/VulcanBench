"""scripts/frontier-v5/board.py on synthetic run records."""

from __future__ import annotations

import csv
import importlib.util
import json
import math
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "frontier-v5" / "board.py"
_spec = importlib.util.spec_from_file_location("v5_board", SCRIPT)
assert _spec and _spec.loader
board = importlib.util.module_from_spec(_spec)
sys.modules["v5_board"] = board
_spec.loader.exec_module(board)

TASKS = list(
    json.loads((REPO / "scripts" / "frontier-v5" / "progress_rule.json").read_text())["tasks"]
)


def rec(
    task: str,
    i: int,
    *,
    ref="sol62",
    model="gpt-6.2-sol",
    agent="codex",
    effort="max",
    passed=False,
    progress=0.0,
    minutes=20.0,
    cost=1.0,
    concurrency=1,
    scored=True,
    flags=(),
    checksum="c1",
) -> dict:
    return {
        "schema": "vulcanbench-frontier-v5-run/1",
        "job": f"{ref}-{effort}-{task}",
        "trial": f"{task}__{i}",
        "task": task,
        "task_checksum": checksum,
        "ref": ref,
        "agent": agent,
        "agent_version": "0.166.0",
        "model": model,
        "effort": effort,
        "attempt": i,
        "concurrency": concurrency,
        "agent_minutes": minutes,
        "scored": scored,
        "passed": passed if scored else None,
        "reward": int(passed) if scored else None,
        "progress": progress if scored else None,
        "flags": list(flags),
        "cost": {"api_equivalent_usd": cost},
    }


def write(root: Path, runs: list[dict]) -> Path:
    for r in runs:
        d = root / r["job"] / r["trial"]
        d.mkdir(parents=True)
        (d / "run.json").write_text(json.dumps(r))
    return root


def full_column(**kw) -> list[dict]:
    # Task k: k of the 3 attempts pass (capped at 3); progress 1 on a pass, 0.5 otherwise.
    runs = []
    for k, t in enumerate(TASKS):
        for i in range(3):
            p = i < min(k, 3)
            runs.append(
                rec(t, i + 1, passed=p, progress=1.0 if p else 0.5, minutes=10.0 + k + i, **kw)
            )
    return runs


def test_row_measures_and_stderr(tmp_path: Path) -> None:
    runs = full_column()
    rows, tasks = board.aggregate(runs, TASKS, 3)
    (r,) = rows
    per_task = [min(k, 3) / 3 for k in range(len(TASKS))]
    assert r["n"] == 18 and r["n_tasks"] == 6 and r["passes"] == sum(min(k, 3) for k in range(6))
    assert r["pass_rate"] == round(r["passes"] / 18, 4)
    mean = sum(per_task) / 6
    se = math.sqrt(sum((x - mean) ** 2 for x in per_task) / 5 / 6)
    assert r["pass_rate_se"] == round(se, 4)
    prog_task = [(min(k, 3) * 1.0 + (3 - min(k, 3)) * 0.5) / 3 for k in range(6)]
    assert r["progress"] == round(sum(prog_task) / 6, 4)
    assert r["median_agent_minutes"] == 13.5 and r["n_speed"] == 18
    assert r["cost_total_usd"] == 18.0 and r["cost_per_attempt_usd"] == 1.0
    assert not r["partial"] and r["excluded"] == 0
    assert [t["task"] for t in tasks] == TASKS
    assert (
        tasks[3]["pass_rate"] == 1.0
        and tasks[0]["pass_rate"] == 0.0
        and tasks[0]["progress"] == 0.5
    )


def test_unscored_runs_are_excluded_not_zeros_and_mark_partial(tmp_path: Path) -> None:
    runs = full_column()
    runs[0] = rec(TASKS[0], 1, scored=False)
    rows, tasks = board.aggregate(runs, TASKS, 3)
    (r,) = rows
    assert r["n"] == 17 and r["excluded"] == 1 and r["partial"]
    assert tasks[0]["n"] == 2


def test_speed_uses_serial_runs_only() -> None:
    runs = [
        rec(TASKS[0], 1, minutes=10, concurrency=2),
        rec(TASKS[0], 2, minutes=30, concurrency=1),
    ]
    (r,), _ = board.aggregate(runs, TASKS, 3)
    assert r["median_agent_minutes"] == 30 and r["n_speed"] == 1
    runs = [rec(TASKS[0], 1, concurrency=2)]
    (r,), _ = board.aggregate(runs, TASKS, 3)
    assert r["median_agent_minutes"] is None and r["pass_rate_se"] is None  # one task


def test_rows_sort_by_pass_rate_then_progress_and_count_flags() -> None:
    a = [rec(t, 1, ref="terra", model="gpt-6.1-terra", passed=False, progress=0.9) for t in TASKS]
    b = [rec(t, 1, ref="dsflash", model="deepseek-flash", agent="dsh", progress=0.2) for t in TASKS]
    c = [
        rec(t, 1, passed=True, progress=1.0, flags=("safety-refusal",) if t == TASKS[0] else ())
        for t in TASKS
    ]
    rows, _ = board.aggregate(a + b + c, TASKS, 1)
    assert [r["model"] for r in rows] == ["gpt-6.2-sol", "gpt-6.1-terra", "deepseek-flash"]
    assert rows[0]["flags"] == {"safety-refusal": 1}


def test_runs_outside_the_board_tasks_are_ignored() -> None:
    runs = [rec("ferrycore-crossing-opaque-parity", 1, passed=True), rec(TASKS[0], 1)]
    (r,), _ = board.aggregate(runs, TASKS, 3)
    assert r["n"] == 1 and r["passes"] == 0


def test_cli_writes_json_and_csv(tmp_path: Path, capsys) -> None:
    runs = [*full_column(), rec(TASKS[1], 4, checksum="c2")]
    root = write(tmp_path / "runs", runs)
    out = tmp_path / "board"
    assert board.main([str(root), "--out", str(out)]) == 0
    b = json.loads((out / "board.json").read_text())
    assert b["suite"] == "frontier-v5" and b["tasks"] == TASKS and len(b["rows"]) == 1
    assert b["suite_version"] and b["progress_rule_sha256"]
    assert any(n.startswith("mixed task versions") for n in b["notes"])
    with (out / "board.csv").open() as fh:
        rows = list(csv.DictReader(fh))
    assert rows[0]["model"] == "gpt-6.2-sol" and rows[0]["n"] == "19" and rows[0]["flags"] == "{}"
    with (out / "board-tasks.csv").open() as fh:
        assert len(list(csv.DictReader(fh))) == 6
    assert "gpt-6.2-sol max: pass" in capsys.readouterr().out


def test_cli_without_runs_fails(tmp_path: Path) -> None:
    (tmp_path / "empty").mkdir()
    assert board.main([str(tmp_path / "empty"), "--out", str(tmp_path / "o")]) == 1


@pytest.mark.parametrize("vals,expected", [([], None), ([0.5], None), ([0.0, 1.0], 0.5)])
def test_se(vals: list[float], expected: float | None) -> None:
    assert board._se(vals) == expected


def test_a_missing_checksum_is_not_a_task_version() -> None:
    runs = [rec(TASKS[0], 1), rec(TASKS[0], 2, checksum=None, scored=False)]
    _, tasks = board.aggregate(runs, TASKS, 3)
    assert tasks[0]["task_checksums"] == ["c1"]

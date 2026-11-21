"""scripts/frontier-v5/grid_board.py on synthetic grid run records."""

from __future__ import annotations

import csv
import importlib.util
import json
import math
import sys
from pathlib import Path
from typing import Any

import pytest

REPO = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location(
    "v5_grid_board", REPO / "scripts" / "frontier-v5" / "grid_board.py"
)
assert _spec and _spec.loader
gb = importlib.util.module_from_spec(_spec)
sys.modules["v5_grid_board"] = gb
_spec.loader.exec_module(gb)

GRID = {
    "cli11-grid": {"language": "cpp", "tier": "smaller"},
    "black-grid": {"language": "python", "tier": "smaller"},
    "mpmath-grid": {"language": "python", "tier": "larger"},
    "curl-grid": {"language": "c", "tier": "larger"},
}


def rec(
    task: str,
    done: int,
    total: int = 8,
    *,
    ref: str = "sol62",
    model: str = "gpt-6.2-sol",
    agent: str = "codex",
    effort: str = "max",
    scored: bool = True,
    budget: bool = True,
    tamper: bool = True,
    cost: float | None = 2.0,
    flags: tuple[str, ...] = (),
    kinds: dict[str, list[int]] | None = None,
    checksum: str = "c1",
    trial: str | None = None,
    budget_sec: float | None = None,
) -> dict[str, Any]:
    return {
        "schema": "vulcanbench-frontier-v5-run/2",
        "job": f"{ref}-{effort}-grid-b1",
        "trial": trial or f"{task}__x",
        "task": task,
        "task_checksum": checksum,
        "ref": ref,
        "agent": agent,
        "agent_version": "0.166.0",
        "model": model,
        "effort": effort,
        "scored": scored,
        "items_done": done if scored else None,
        "items_total": total if scored else None,
        "items_by_kind": kinds or ({"defect": [done, total]} if scored else None),
        "budget_reached": budget,
        "budget_sec": budget_sec,
        "tamper_clean": tamper,
        "guards_failed": 0,
        "agent_minutes": 20.0,
        "flags": list(flags),
        "tokens": {"input": 1000, "cached_input": 800, "output": 50},
        "cost": {"api_equivalent_usd": cost},
    }


def full(**kw: Any) -> list[dict[str, Any]]:
    return [
        rec("cli11-grid", 6, **kw),
        rec("black-grid", 2, **kw),
        rec("mpmath-grid", 4, total=10, **kw),
        rec("curl-grid", 0, tamper=False, **kw),
    ]


def test_headline_tiers_languages_cells_and_economics() -> None:
    (r,), cells = gb.aggregate(full(), GRID)
    assert (r["items_done"], r["items_total"], r["n_envs"]) == (12, 34, 4)
    p = 12 / 34
    assert r["rate"] == round(p, 4) and r["rate_se"] == round(math.sqrt(p * (1 - p) / 34), 4)
    assert r["tiers"]["smaller"]["done"] == 8 and r["tiers"]["smaller"]["total"] == 16
    assert r["tiers"]["larger"] == gb.binomial(4, 18)
    assert r["languages"]["python"] == gb.binomial(6, 18) and "rust" not in r["languages"]
    assert r["cells"] == {
        "cpp": {"smaller": {"done": 6, "total": 8}},
        "python": {"smaller": {"done": 2, "total": 8}, "larger": {"done": 4, "total": 10}},
        "c": {"larger": {"done": 0, "total": 8}},
    }
    assert r["kinds"]["defect"] == gb.binomial(12, 34)
    assert (r["tokens_input"], r["tokens_cached_input"], r["tokens_output"]) == (4000, 3200, 200)
    assert r["cost_total_usd"] == 8.0 and r["cost_per_run_usd"] == 2.0 and r["n_cost"] == 4
    assert not r["partial"] and r["excluded"] == 0
    assert r["budget_reached"] == 4 and r["tamper_flagged"] == 1
    assert [c["task"] for c in cells] == list(GRID)
    assert cells[3]["tamper_clean"] is False and cells[2]["tier"] == "larger"
    assert "median_agent_minutes" not in r  # no speed column (R13)


def test_unscored_runs_are_excluded_and_mark_partial() -> None:
    runs = full()
    runs[1] = rec("black-grid", 0, scored=False)
    (r,), cells = gb.aggregate(runs, GRID)
    assert r["excluded"] == 1 and r["partial"] and r["n_envs"] == 3
    assert r["items_total"] == 26
    assert cells[1]["scored"] is False and cells[1]["items_done"] is None


def test_two_scored_runs_on_one_environment_are_refused() -> None:
    runs = [*full(), rec("cli11-grid", 8, trial="cli11-grid__rerun")]
    with pytest.raises(gb.BoardError, match="two scored runs on cli11-grid"):
        gb.aggregate(runs, GRID)


def test_each_budget_is_its_own_row() -> None:
    # The pilot's calibration: one model on one environment at 15 and 30 minutes.
    runs = [
        rec("cli11-grid", 5, budget_sec=900.0, trial="a"),
        rec("cli11-grid", 7, budget_sec=1800.0, trial="b"),
    ]
    rows, cells = gb.aggregate(runs, GRID)
    assert [(r["budget_min"], r["items_done"]) for r in rows] == [(30.0, 7), (15.0, 5)]
    assert [c["budget_min"] for c in cells if c["task"] == "cli11-grid"] == [30.0, 15.0]
    with pytest.raises(gb.BoardError, match=r"budget 15\.0 min: two scored runs"):
        gb.aggregate([*runs, rec("cli11-grid", 6, budget_sec=900.0, trial="c")], GRID)


def test_rows_sort_by_rate_and_count_flags() -> None:
    a = full(ref="terra", model="gpt-6.1-terra")
    b = [
        rec(
            t,
            8,
            ref="fable52",
            model="claude-fable-5-2",
            agent="claude-code",
            flags=("safety-refusal",) if t == "curl-grid" else (),
        )
        for t in GRID
    ]
    rows, cells = gb.aggregate(a + b, GRID)
    assert [x["model"] for x in rows] == ["claude-fable-5-2", "gpt-6.1-terra"]
    assert rows[0]["flags"] == {"safety-refusal": 1} and rows[0]["rate"] == 1.0
    assert [c["model"] for c in cells[:4]] == ["claude-fable-5-2"] * 4


def test_binomial_edges() -> None:
    assert gb.binomial(0, 0) == {"done": 0, "total": 0, "rate": None, "se": None}
    assert gb.binomial(5, 5)["se"] == 0.0


def test_runs_outside_the_grid_and_old_schema_are_ignored(tmp_path: Path) -> None:
    runs = [*full(), rec("ferrycore-crossing-opaque-parity", 8)]
    (r,), _ = gb.aggregate(runs, GRID)
    assert r["items_total"] == 34
    d = tmp_path / "runs" / "j" / "t"
    d.mkdir(parents=True)
    (d / "run.json").write_text(json.dumps({"schema": "vulcanbench-frontier-v5-run/1"}))
    assert gb.load_runs([tmp_path / "runs"]) == []


def write_grid(path: Path) -> Path:
    path.write_text(
        json.dumps({"schema": 1, "budget_min": 20, "environments": GRID, "batches": [list(GRID)]})
    )
    return path


def test_cli_writes_json_and_csv(tmp_path: Path, capsys) -> None:
    runs = [*full(), rec("cli11-grid", 2, ref="terra", model="gpt-6.1-terra", checksum="c2")]
    for i, r in enumerate(runs):
        d = tmp_path / "runs" / r["job"] / f"{r['trial']}{i}"
        d.mkdir(parents=True)
        (d / "run.json").write_text(json.dumps(r))
    out = tmp_path / "board"
    g = write_grid(tmp_path / "grid.json")
    assert gb.main([str(tmp_path / "runs"), "--grid", str(g), "--out", str(out)]) == 0
    b = json.loads((out / "grid-board.json").read_text())
    assert b["suite"] == "frontier-v5" and len(b["rows"]) == 2 and len(b["cells"]) == 8
    assert b["environments"] == GRID
    assert any(n.startswith("environment versions differ") for n in b["notes"])
    with (out / "grid-board.csv").open() as fh:
        rows = list(csv.DictReader(fh))
    assert rows[0]["model"] == "gpt-6.2-sol" and rows[0]["items_total"] == "34"
    assert json.loads(rows[0]["cells"])["cpp"]["smaller"] == {"done": 6, "total": 8}
    assert rows[1]["partial"] == "True"
    with (out / "grid-cells.csv").open() as fh:
        assert len(list(csv.DictReader(fh))) == 8
    assert "gpt-6.2-sol max None min: 12/34 items" in capsys.readouterr().out


def test_cli_errors(tmp_path: Path, capsys) -> None:
    g = write_grid(tmp_path / "grid.json")
    (tmp_path / "empty").mkdir()
    assert gb.main([str(tmp_path / "empty"), "--grid", str(g), "--out", str(tmp_path / "o")]) == 1
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps({"environments": {"x": {"language": "go", "tier": "smaller"}}}))
    assert gb.main([str(tmp_path / "empty"), "--grid", str(bad), "--out", str(tmp_path / "o")]) == 1
    assert "bad language or tier" in capsys.readouterr().err

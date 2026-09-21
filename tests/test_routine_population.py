"""Routine v1 judging population: the Devin solver receipt and the builder's bookkeeping."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

from harness.solver_receipts import solver_receipt

ROOT = Path(__file__).resolve().parents[1]


def _builder():
    spec = importlib.util.spec_from_file_location(
        "build_routine_population", ROOT / "scripts/cii-v4-board/build_routine_population.py"
    )
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _devin_run(tmp_path: Path, usage: dict) -> Path:
    run = tmp_path / "run"
    run.mkdir()
    (run / "cli-agent-stream.jsonl").write_text('{"type": "stdout", "text": "done"}\n')
    (run / "trace.jsonl").write_text(
        json.dumps({"type": "task_start", "data": {}})
        + "\n"
        + json.dumps({"type": "devin_usage", "data": usage})
        + "\n"
    )
    return run


USAGE = {
    "requests": 3,
    "input_tokens": 1000,
    "output_tokens": 200,
    "cache_read_tokens": 500,
    "cache_creation_tokens": 0,
    "served_models": {"swe-2-high": 3},
    "total_credit_cost": 0,
    "total_acu_cost": 0.0,
}


def test_devin_receipt_matches_summary(tmp_path: Path) -> None:
    run = _devin_run(tmp_path, USAGE)
    summary = {"model": "devin:swe-2", "tokens": {"prompt": 1500, "completion": 200}}
    receipt = solver_receipt(run, summary)
    assert receipt["raw_tokens"] == 1700
    assert receipt["result_receipts"] == 3
    assert receipt["served_models"] == {"swe-2-high": 3}
    assert len(receipt["stream_sha256"]) == 64


def test_devin_receipt_mismatch_is_refused(tmp_path: Path) -> None:
    run = _devin_run(tmp_path, USAGE)
    summary = {"model": "devin:swe-2", "tokens": {"prompt": 1499, "completion": 200}}
    with pytest.raises(ValueError, match="mismatch"):
        solver_receipt(run, summary)


def test_builder_keeps_latest_run_and_lists_gaps(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    builder = _builder()
    tasks = tmp_path / "tasks"
    for name in ("t-one", "t-two", "t-three"):
        (tasks / name).mkdir(parents=True)
        (tasks / name / "metadata.json").write_text("{}")
    runs = tmp_path / "runs"

    def add_run(name: str, task: str, finished_at: str, finished: bool = True) -> None:
        d = runs / "runs-x" / "low" / name
        d.mkdir(parents=True)
        (d / "summary.json").write_text(
            json.dumps(
                {
                    "model": "codex:m",
                    "task_id": task,
                    "finished": finished,
                    "duration_s": 10.0,
                    "total_tokens": 5,
                    "started_at": "2026-09-20T00:00:00",
                    "finished_at": finished_at,
                    "scores": {"functional": 1.0, "quality": 0.9, "security": 1.0},
                    "cli_agent": {"harness_version": "codex-cli 1"},
                    "economics": {"api_equivalent_cost_usd": None},
                }
            )
        )

    add_run("a-old", "t-one", "2026-09-20T01:00:00")
    add_run("a-new", "t-one", "2026-09-20T02:00:00")
    add_run("b-unfinished", "t-two", "2026-09-20T03:00:00", finished=False)

    def fake_inputs(run: Path, _tasks: Path) -> dict:
        summary = json.loads((run / "summary.json").read_text())
        if not summary["finished"]:
            raise ValueError(f"Incomplete source run: {run}")
        return {"source_hashes": {"summary": "s", "patch": "p", "issue": "i", "task": "t"}}

    monkeypatch.setattr(builder.base, "inputs", fake_inputs)
    monkeypatch.setattr(builder, "solver_receipt", lambda run, summary: {"raw_tokens": 5})
    record = builder.build({"x": ("runs-x", ("low",), "codex:m", "Model X")}, runs, tasks)
    assert [r["run_id"] for r in record["rows"]] == ["a-new"]
    assert [d["run_id"] for d in record["duplicates"]] == ["a-old"]
    assert [(e["task"], e["run_id"]) for e in record["excluded"]] == [("t-two", "b-unfinished")]
    assert [m["task"] for m in record["missing"]] == ["t-three"]
    assert record["cells"] == {"x/low": 1}
    # rows + excluded + missing accounts for every task in the cell
    assert len(record["rows"]) + len(record["excluded"]) + len(record["missing"]) == 3

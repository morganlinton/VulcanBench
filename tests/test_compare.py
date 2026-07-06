"""Tests for the cached-run comparison / freeze-baselines flow."""

from __future__ import annotations

import json
from pathlib import Path

from harness.compare import build_matrix, fresh_run_counts, suite_version
from harness.tasks import load_task, task_hash


def _make_task(suite: Path, task_id: str, expected: int = 2) -> None:
    task = suite / task_id
    (task / "repo").mkdir(parents=True)
    (task / "tests").mkdir(parents=True)
    (task / "repo" / "m.py").write_text("def f():\n    return 0\n")
    (task / "tests" / "t.py").write_text(
        f"from m import f\n\ndef test_it():\n    assert f() == {expected}\n"
    )
    (task / "metadata.json").write_text(
        json.dumps(
            {
                "id": task_id,
                "category": "bug_fix",
                "languages": ["python"],
                "difficulty": "trivial",
                "created": "2026-07-05",
                "source": "hand-authored",
                "decontamination_notes": "fixture",
                "tests": {
                    "fail_to_pass": [{"name": "it", "cmd": "python -m pytest t.py -q"}],
                    "pass_to_pass": [],
                },
            }
        )
    )
    (task / "issue.md").write_text("fix f")


def _make_suite(base: Path, name: str, task_ids: list[str]) -> Path:
    suite = base / name
    for tid in task_ids:
        _make_task(suite, tid)
    return suite


def _write_run(
    runs: Path, task_id: str, model: str, effort: str, functional: float, cost: float, thash: str
) -> None:
    base = f"{task_id}-{model}-{effort}".replace(":", "_")
    d = runs / base
    n = 1
    while d.exists():
        d = runs / f"{base}-{n}"
        n += 1
    d.mkdir(parents=True)
    (d / "summary.json").write_text(
        json.dumps(
            {
                "run_id": d.name,
                "task_id": task_id,
                "model": model,
                "effort": {"requested": effort},
                "scores": {"functional": functional},
                "cost_usd": cost,
                "task_hash": thash,
            }
        )
    )


def test_suite_version_changes_with_task_definition(tmp_path: Path) -> None:
    _make_suite(tmp_path, "s", ["a", "b"])
    v1 = suite_version("s", tmp_path)["version"]
    # Re-reading the same tasks yields the same version.
    assert suite_version("s", tmp_path)["version"] == v1
    # Changing a task's test changes its hash -> the suite version changes.
    (tmp_path / "s" / "a" / "tests" / "t.py").write_text(
        "from m import f\n\ndef test_it():\n    assert f() == 3\n"
    )
    assert suite_version("s", tmp_path)["version"] != v1


def test_build_matrix_assembles_cells_from_cache(tmp_path: Path) -> None:
    suite = _make_suite(tmp_path, "s", ["a", "b"])
    ha = task_hash(load_task("a", suite))
    hb = task_hash(load_task("b", suite))
    runs = tmp_path / "runs"

    # Opus solves both at low; Fable solves one of two.
    _write_run(runs, "a", "opus", "low", 1.0, 0.5, ha)
    _write_run(runs, "b", "opus", "low", 1.0, 0.7, hb)
    _write_run(runs, "a", "fable", "low", 1.0, 2.0, ha)
    _write_run(runs, "b", "fable", "low", 0.0, 3.0, hb)

    m = build_matrix("s", runs_dir=runs, tasks_base=tmp_path)
    cells = {(c["model"], c["effort"]): c for c in m["cells"]}

    opus = cells[("opus", "low")]
    assert opus["complete"] is True
    assert opus["pass_at_1"] == 1.0
    assert opus["cost_usd"] == 1.2  # 0.5 + 0.7

    fable = cells[("fable", "low")]
    assert fable["complete"] is True
    assert fable["pass_at_1"] == 0.5  # 1 of 2 tasks solved
    assert m["n_stale_excluded"] == 0


def test_stale_runs_are_excluded_when_a_task_changes(tmp_path: Path) -> None:
    suite = _make_suite(tmp_path, "s", ["a", "b"])
    ha = task_hash(load_task("a", suite))
    hb = task_hash(load_task("b", suite))
    runs = tmp_path / "runs"
    _write_run(runs, "a", "opus", "low", 1.0, 0.5, ha)
    _write_run(runs, "b", "opus", "low", 1.0, 0.7, hb)

    # Recalibrate task "a": its recorded run is now stale and must be dropped,
    # leaving the opus cell incomplete (only b covered).
    (suite / "a" / "tests" / "t.py").write_text(
        "from m import f\n\ndef test_it():\n    assert f() == 99\n"
    )

    m = build_matrix("s", runs_dir=runs, tasks_base=tmp_path)
    assert m["n_stale_excluded"] == 1
    opus = next(c for c in m["cells"] if c["model"] == "opus")
    assert opus["complete"] is False
    assert opus["n_present"] == 1
    assert opus["missing_tasks"] == ["a"]


def test_new_model_is_a_single_new_column(tmp_path: Path) -> None:
    """The core value: baselines come from cache; a new model is one added cell."""
    suite = _make_suite(tmp_path, "s", ["a", "b"])
    ha = task_hash(load_task("a", suite))
    hb = task_hash(load_task("b", suite))
    runs = tmp_path / "runs"
    # Cached baseline (opus) — never re-run.
    _write_run(runs, "a", "opus", "high", 1.0, 0.5, ha)
    _write_run(runs, "b", "opus", "high", 1.0, 0.7, hb)
    assert len(build_matrix("s", runs_dir=runs, tasks_base=tmp_path)["cells"]) == 1

    # Add a new model's runs; the baseline is untouched, the matrix gains one cell.
    _write_run(runs, "a", "newmodel", "high", 1.0, 4.0, ha)
    _write_run(runs, "b", "newmodel", "high", 0.0, 5.0, hb)
    cells = build_matrix("s", runs_dir=runs, tasks_base=tmp_path)["cells"]
    assert len(cells) == 2
    assert {c["model"] for c in cells} == {"opus", "newmodel"}


def test_fresh_run_counts_filters_model_effort_and_staleness(tmp_path: Path) -> None:
    suite = _make_suite(tmp_path, "s", ["a", "b"])
    ha = task_hash(load_task("a", suite))
    hb = task_hash(load_task("b", suite))
    runs = tmp_path / "runs"

    _write_run(runs, "a", "opus", "low", 1.0, 0.5, ha)
    _write_run(runs, "a", "opus", "low", 1.0, 0.5, ha)  # a second attempt for a
    _write_run(runs, "b", "opus", "low", 1.0, 0.7, hb)
    _write_run(runs, "a", "opus", "high", 1.0, 0.9, ha)  # different effort
    _write_run(runs, "a", "fable", "low", 1.0, 2.0, ha)  # different model
    _write_run(runs, "b", "opus", "low", 1.0, 0.7, "STALE")  # stale hash

    counts = fresh_run_counts(["a", "b"], "opus", "low", runs, suite)
    assert counts["a"] == 2  # two fresh opus-low runs for a
    assert counts["b"] == 1  # the stale one is not counted

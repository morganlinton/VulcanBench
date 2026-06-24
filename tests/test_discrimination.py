"""Tests for the discrimination analysis (harness.report.build_discrimination)."""

from __future__ import annotations

from harness.report import _discrimination_markdown, build_discrimination


def _row(task_id: str, model: str, functional: float) -> dict:
    return {"task_id": task_id, "model": model, "functional": functional}


def test_needs_two_models() -> None:
    rows = [_row("t1", "m1", 1.0), _row("t2", "m1", 0.0)]
    disc = build_discrimination(rows)
    assert disc["available"] is False
    assert disc["model_pairs"] == []


def test_buckets_all_pass_all_fail_and_discriminating() -> None:
    rows = [
        # everyone passes -> no signal
        _row("easy", "A", 1.0),
        _row("easy", "B", 1.0),
        # everyone fails -> no signal
        _row("broken", "A", 0.0),
        _row("broken", "B", 0.0),
        # split -> discriminating
        _row("hard", "A", 1.0),
        _row("hard", "B", 0.0),
    ]
    disc = build_discrimination(rows)
    assert disc["available"] is True
    assert disc["all_pass"] == 1
    assert disc["all_fail"] == 1
    assert disc["discriminating"] == 1
    assert disc["no_signal_tasks"] == ["broken", "easy"]


def test_pairwise_separation_counts() -> None:
    rows = [
        _row("t1", "A", 1.0),
        _row("t1", "B", 0.0),  # A only
        _row("t2", "A", 0.0),
        _row("t2", "B", 1.0),  # B only
        _row("t3", "A", 1.0),
        _row("t3", "B", 1.0),  # agree
    ]
    disc = build_discrimination(rows)
    (pair,) = disc["model_pairs"]
    assert pair["a"] == "A" and pair["b"] == "B"
    assert pair["common_tasks"] == 3
    assert pair["a_solves_b_fails"] == 1
    assert pair["b_solves_a_fails"] == 1
    assert pair["separating"] == 2
    assert pair["agree"] == 1


def test_identical_models_separate_on_nothing() -> None:
    # Two models that pass and fail exactly the same tasks: 0 separating.
    rows = []
    for i, fn in enumerate([1.0, 1.0, 0.0, 0.0]):
        rows += [_row(f"t{i}", "A", fn), _row(f"t{i}", "B", fn)]
    disc = build_discrimination(rows)
    (pair,) = disc["model_pairs"]
    assert pair["separating"] == 0
    md = "\n".join(_discrimination_markdown(disc))
    assert "cannot be told apart" in md


def test_repeats_use_majority_threshold() -> None:
    # 1/3 solves is below the 0.5 threshold -> counts as fail for A.
    rows = [
        _row("t", "A", 1.0),
        _row("t", "A", 0.0),
        _row("t", "A", 0.0),
        _row("t", "B", 1.0),
        _row("t", "B", 1.0),
        _row("t", "B", 0.0),
    ]
    disc = build_discrimination(rows)
    entry = disc["tasks"][0]
    assert entry["solve_rates"]["A"] < 0.5
    assert entry["solve_rates"]["B"] >= 0.5
    assert entry["separates"] is True

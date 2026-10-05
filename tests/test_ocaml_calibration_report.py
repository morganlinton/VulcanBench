"""Prevent partial coverage or obsolete task definitions from overstating difficulty."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import pytest

from harness.tasks import load_task, task_hash
from scripts.report_ocaml_calibration import build_report, render_model_card


@pytest.mark.parametrize(
    ("suite", "title"),
    [
        ("ocaml-candidates", "OCaml candidates"),
        ("ocaml-incremental-candidate", "OCaml Incremental candidate"),
        ("ocaml-atomic-candidate", "OCaml atomic rewiring candidate"),
        ("ocaml-atomic-candidate-v2", "OCaml atomic rewiring candidate v2"),
        ("ocaml-serialization-candidate", "OCaml typed serialization candidate"),
        ("ocaml-serialization-candidate-v2", "OCaml typed serialization candidate v2"),
        ("ocaml-serialization-candidate-v3", "OCaml typed serialization candidate v3"),
        ("ocaml-union-heldout", "OCaml open union held-out workload"),
        ("ocaml-registry-candidate", "OCaml generative module registry candidate"),
    ],
)
def test_candidate_pool_uses_its_own_identity_and_coverage(
    tmp_path: Path, suite: str, title: str
) -> None:
    base = task_base(tmp_path)
    shutil.copytree(base / "ocaml-v1", base / suite)
    candidate = base / suite / "task-a"
    (candidate / "issue.md").write_text("A separately frozen candidate")
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
        record(base, runs, "task-b", index, 0)
    report = build_report(runs, tasks_base=base, suite=suite)
    assert report["suite"] == suite
    assert report["pass_at_1"] is None
    assert len(report["excluded_runs"]) == 3
    for index in range(3):
        record(base, runs, "task-a", index + 3, 0)
        path = runs / f"task-a-{index + 3}" / "summary.json"
        summary = json.loads(path.read_text())
        summary["task_hash"] = task_hash(load_task("task-a", base / suite))
        path.write_text(json.dumps(summary))
    report = build_report(runs, tasks_base=base, suite=suite)
    assert report["pass_at_1"] == 0
    assert f"<h1>{title} model card</h1>" in render_model_card(report)
    original = build_report(runs, tasks_base=base)
    assert original["suite"] == "ocaml-v1"
    assert original["pass_at_1"] == 0.5


def task_base(tmp_path: Path) -> Path:
    base = tmp_path / "tasks"
    root = base / "ocaml-v1"
    for name in ("task-a", "task-b"):
        task = root / name
        task.mkdir(parents=True)
        (task / "issue.md").write_text(name)
        (task / "metadata.json").write_text(json.dumps({"primary_view": "engineering"}))
    (root / "suite.json").write_text(json.dumps({"tasks": ["task-a", "task-b"]}))
    return base


def record(
    base: Path, runs: Path, name: str, index: int, score: float | None, *, stale: bool = False
) -> None:
    path = runs / f"{name}-{index}"
    path.mkdir(parents=True)
    summary = {
        "task_id": name,
        "task_hash": "obsolete" if stale else task_hash(load_task(name, base / "ocaml-v1")),
        "model": "codex:gpt-6.1-sol",
        "effort": {"requested": "medium"},
        "scores": {"functional": score},
    }
    (path / "summary.json").write_text(json.dumps(summary))


def test_partial_coverage_has_no_suite_score(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 0)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["observed_below_80_percent"] is None
    assert report["status"] == "incomplete calibration"


def test_stale_patches_do_not_change_current_difficulty(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
        record(base, runs, "task-b", index, 1 if index == 0 else 0)
    record(base, runs, "task-a", 99, 0, stale=True)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] == pytest.approx(2 / 3)
    assert report["observed_below_80_percent"] is True
    assert report["excluded_runs"] == [{"run_id": "task-a-99", "reason": "old task revision"}]
    assert report["independent_confirmation"] is False


def test_score_weights_tasks_equally_with_unequal_attempts(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
    for index in range(6):
        record(base, runs, "task-b", index, 0)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] == 0.5


def test_overlapping_views_do_not_duplicate_suite_weight(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    root = base / "ocaml-v1"
    (root / "task-a" / "metadata.json").write_text(json.dumps({"primary_view": "both"}))
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 0)
        record(base, runs, "task-b", index, 1)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] == 0.5
    assert report["views"]["engineering"]["task_ids"] == ["task-a", "task-b"]
    assert report["views"]["engineering"]["pass_at_1"] == 0.5
    assert report["views"]["language-mastery"]["task_ids"] == ["task-a"]
    assert report["views"]["language-mastery"]["pass_at_1"] == 0


def test_complete_view_can_be_reported_while_suite_is_incomplete(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    root = base / "ocaml-v1"
    (root / "task-a" / "metadata.json").write_text(json.dumps({"primary_view": "language-mastery"}))
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["views"]["language-mastery"]["pass_at_1"] == 1
    assert report["views"]["engineering"]["pass_at_1"] is None


def test_failed_check_names_preserve_regression_and_behavior_distinction(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    record(base, runs, "task-a", 0, 0)
    path = runs / "task-a-0" / "summary.json"
    summary = json.loads(path.read_text())
    summary["verifier"] = {
        "fail_to_pass": {"retry": False, "ttl": True},
        "pass_to_pass": {"interfaces": False},
    }
    path.write_text(json.dumps(summary))
    report = build_report(runs, tasks_base=base)
    failed = report["tasks"][0]["runs"][0]["failed_checks"]
    assert failed == {"fail_to_pass": ["retry"], "pass_to_pass": ["interfaces"]}


def test_public_patch_exposure_is_reported_without_changing_score(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    metadata = base / "ocaml-v1" / "task-a" / "metadata.json"
    metadata.write_text(
        json.dumps(
            {
                "primary_view": "both",
                "source": "oss",
                "decontaminated": False,
                "decontamination_notes": "Public patch; training cutoff unknown.",
                "upstream": {"pr": "https://example.com/public-fix"},
            }
        )
    )
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
        record(base, runs, "task-b", index, 0)
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] == 0.5
    public = report["tasks"][0]
    assert public["source"] == "oss"
    assert public["decontaminated"] is False
    assert public["decontamination_notes"] == "Public patch; training cutoff unknown."
    assert public["upstream"] == {"pr": "https://example.com/public-fix"}
    assert report["tasks"][1]["decontaminated"] is None


@pytest.mark.parametrize("omit_score", [False, True])
def test_unscored_receipts_do_not_count_as_failure_or_complete_coverage(
    tmp_path: Path, omit_score: bool
) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 1)
    record(base, runs, "task-b", 0, 0)
    record(base, runs, "task-b", 1, None)
    record(base, runs, "task-b", 2, None)
    if omit_score:
        for index in (1, 2):
            path = runs / f"task-b-{index}" / "summary.json"
            summary = json.loads(path.read_text())
            del summary["scores"]
            path.write_text(json.dumps(summary))
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["tasks"][1]["attempts"] == 1
    assert report["tasks"][1]["pass_at_1"] == 0
    assert [row["run_id"] for row in report["unscored_runs"]] == ["task-b-1", "task-b-2"]
    assert all(row["task_id"] == "task-b" for row in report["unscored_runs"])
    assert report["excluded_runs"] == []


def test_native_and_docker_runs_are_not_combined_into_a_score(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for name, sandbox in (("task-a", "local"), ("task-b", "docker")):
        for index in range(3):
            record(base, runs, name, index, 1)
            path = runs / f"{name}-{index}/summary.json"
            data = json.loads(path.read_text())
            data["manifest"] = {"sandbox": {"mode": sandbox}}
            path.write_text(json.dumps(data))
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["observed_below_80_percent"] is None
    assert report["status"] == "mixed execution environments; select one"
    selected = build_report(runs, tasks_base=base, sandbox="local")
    assert selected["tasks"][0]["attempts"] == 3
    assert selected["tasks"][1]["attempts"] == 0
    assert selected["pass_at_1"] is None
    assert len(selected["excluded_runs"]) == 3


def test_report_supports_other_models_without_reusing_calibration_runs(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for name in ("task-a", "task-b"):
        for index in range(3):
            record(base, runs, name, index, 1)
    report = build_report(runs, tasks_base=base, model="other:model", effort="high")
    assert report["model"] == "other:model"
    assert report["effort"] == "high"
    assert report["pass_at_1"] is None
    assert all(row["attempts"] == 0 for row in report["tasks"])


def test_model_card_keeps_incomplete_coverage_visible_and_escapes_labels(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    report = build_report(tmp_path / "empty", tasks_base=base, model="provider:<script>")
    card = render_model_card(report)
    assert "Task pass@1: Pending coverage" in card
    assert "Coverage: 0/2 tasks with at least 3 attempts" in card
    assert "provider:&lt;script&gt;" in card
    assert "provider:<script>" not in card
    assert "data:image/png;base64," in card
    assert "Frozen task hashes" in card


def test_trace_only_abort_is_visible_without_becoming_a_scored_attempt(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    failed = runs / "task-b-aborted"
    failed.mkdir(parents=True)
    (failed / "trace.jsonl").write_text(
        json.dumps(
            {
                "type": "task_start",
                "task_id": "task-b",
                "model": "codex:gpt-6.1-sol",
                "ts": "2026-10-01T09:00:00+00:00",
                "data": {"metadata": {}},
            }
        )
        + "\n"
    )
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["tasks"][1]["attempts"] == 0
    assert report["unscored_runs"] == []
    assert report["trace_only_invocations"][0]["run_id"] == "task-b-aborted"
    assert report["trace_only_invocations"][0]["counts_toward_coverage"] is False
    assert "Trace-only invocations needing identity audit: 1" in render_model_card(report)


@pytest.mark.parametrize("override", [False, True])
def test_changed_solve_budgets_do_not_make_the_standard_column_harder(
    tmp_path: Path, override: bool
) -> None:
    base = task_base(tmp_path)
    root = base / "ocaml-v1/task-a/metadata.json"
    root.write_text(
        json.dumps(
            {
                "primary_view": "engineering",
                "agent_hints": {
                    "suggested_timeout_s": 10800,
                    "suggested_max_steps": 540,
                },
            }
        )
    )
    runs = tmp_path / "runs"
    for name in ("task-a", "task-b"):
        for index in range(3):
            record(base, runs, name, index, 0)
    path = runs / "task-a-0/summary.json"
    data = json.loads(path.read_text())
    if override:
        data["budget_override"] = {"timeout_s": 10800, "max_steps": 540}
    else:
        data["run_limits"] = {"agent_timeout_s": 600, "configured_max_steps": 540}
    path.write_text(json.dumps(data))
    report = build_report(runs, tasks_base=base)
    assert report["tasks"][0]["attempts"] == 2
    assert report["pass_at_1"] is None
    assert len(report["excluded_runs"]) == 1


@pytest.mark.parametrize("eligibility", [False, None, "true"])
def test_reviewed_infrastructure_or_exposure_does_not_manufacture_difficulty(
    tmp_path: Path, eligibility: object
) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for index in range(3):
        record(base, runs, "task-a", index, 0 if index == 0 else 1)
        record(base, runs, "task-b", index, 1)
    raw = runs / "task-a-0/summary.json"
    original = raw.read_bytes()
    (raw.parent / "CALIBRATION_ELIGIBILITY.json").write_text(
        json.dumps({"eligible": eligibility, "reason": "solver tools unavailable"})
    )
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] is None
    assert report["tasks"][0]["attempts"] == 2
    assert report["excluded_runs"] == [{"run_id": "task-a-0", "reason": "solver tools unavailable"}]
    record(base, runs, "task-a", 3, 0)
    complete = build_report(runs, tasks_base=base)
    assert complete["pass_at_1"] == pytest.approx(5 / 6)
    assert complete["observed_below_80_percent"] is False
    assert raw.read_bytes() == original


def test_explicit_eligible_review_keeps_the_recorded_result(tmp_path: Path) -> None:
    base = task_base(tmp_path)
    runs = tmp_path / "runs"
    for name in ("task-a", "task-b"):
        for index in range(3):
            record(base, runs, name, index, 0)
    (runs / "task-a-0/CALIBRATION_ELIGIBILITY.json").write_text(
        json.dumps({"eligible": True, "reason": "review completed"})
    )
    report = build_report(runs, tasks_base=base)
    assert report["pass_at_1"] == 0
    assert report["excluded_runs"] == []

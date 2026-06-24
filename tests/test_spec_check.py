"""Tests for the under-specification checks (harness.spec_check)."""

from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import pytest

from harness.spec_check import (
    FAIL,
    OK,
    WARN,
    gold_changed_line_count,
    has_behavior_cue,
    solvability_verdict,
    static_spec_lint,
)
from harness.tasks import load_task


def _issue(text: str) -> SimpleNamespace:
    return SimpleNamespace(issue=text)


# --- static lint -----------------------------------------------------------


def test_lint_warns_on_defect_only_issue() -> None:
    # The exact shape of the v1 broken scaffolds: a fix is asked for, but the
    # expected behavior is never stated, so the hidden test (run(2) == 4) is
    # unguessable.
    res = static_spec_lint(_issue("# Fix pkg3\n\nCorrect `run` in service module."))
    assert res.status == WARN
    assert "expected behavior" in res.reasons[0]


def test_lint_warns_on_empty_issue() -> None:
    assert static_spec_lint(_issue("")).status == WARN
    assert static_spec_lint(_issue("   \n")).status == WARN


@pytest.mark.parametrize(
    "text",
    [
        "`run` must double its input. Fix `pkg0/service.py`.",  # re-spec'd anchor
        "`Double` increments instead of doubling. Fix `repo/pkg4/calc.go`.",
        "Normalize ratios in `ledger/core.py` so parts sum to the total.",
        "URL-decode captures in `src/router.ts`.",
        "Fix non-deterministic label ordering in `metrics/labels.go`.",
        "Pop should return the most recently pushed value.",
    ],
)
def test_lint_passes_specified_issues(text: str) -> None:
    assert static_spec_lint(_issue(text)).status == OK


def test_has_behavior_cue() -> None:
    assert has_behavior_cue("the function must return 2")
    assert has_behavior_cue("want: [][]string{{`a\"b`}}")
    assert not has_behavior_cue("Correct `run` in service module.")


def test_respecced_anchors_and_good_twin_pass_lint() -> None:
    # The two kept-and-re-spec'd Python anchors plus the already-good twin must
    # all read as specified now.
    for task_id in ("oss-py-m2-00", "oss-py-m3-00", "oss-py-config-merge"):
        task = load_task(task_id, Path("tasks/v1"))
        assert static_spec_lint(task).status == OK, task_id


# --- solvability gate ------------------------------------------------------


def test_gold_changed_line_count_ignores_diff_headers() -> None:
    diff = (
        "diff --git a/m.py b/m.py\n"
        "--- a/m.py\n"
        "+++ b/m.py\n"
        "@@ -1,2 +1,2 @@\n"
        " def run(x):\n"
        "-    return x + 1\n"
        "+    return x * 2\n"
    )
    assert gold_changed_line_count(diff) == 2


def test_verdict_ok_when_model_solves() -> None:
    res = solvability_verdict(solved=2, attempts=5, gold_changed_lines=2, complexity="localized")
    assert res.status == OK


def test_verdict_ok_when_no_attempts() -> None:
    res = solvability_verdict(solved=0, attempts=0, gold_changed_lines=2, complexity="localized")
    assert res.status == OK


def test_verdict_fails_trivial_localized_unsolved() -> None:
    # 0/5 on a 2-line localized fix => under-specified, not hard.
    res = solvability_verdict(solved=0, attempts=5, gold_changed_lines=2, complexity="localized")
    assert res.status == FAIL
    assert "under-specified" in res.reasons[0]


def test_verdict_warns_when_fix_is_large() -> None:
    # 0/5 but the gold fix is big => could be genuinely hard, only warn.
    res = solvability_verdict(solved=0, attempts=5, gold_changed_lines=40, complexity="localized")
    assert res.status == WARN


def test_verdict_warns_when_not_localized() -> None:
    res = solvability_verdict(solved=0, attempts=5, gold_changed_lines=2, complexity="multi_file")
    assert res.status == WARN

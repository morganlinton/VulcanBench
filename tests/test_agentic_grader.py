"""Tests for the agentic correctness grader and its integration."""

from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from harness.agent.providers import LLMResponse, TokenUsage, get_provider
from harness.evaluator.agentic_grader import GRADER_SENTINEL, grade_correctness, grade_rubric
from harness.evaluator.evaluate import evaluate_run
from harness.spec_check import OK, static_spec_lint

_DIFF = "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-old\n+new\n"

_RUBRIC = {
    "blocking": ["is correct", "stays in scope"],
    "weighted": [
        {"weight": 3, "criterion": "reuses the helper"},
        {"weight": 2, "criterion": "raises the right error"},
        {"weight": 2, "criterion": "reuses the parser"},
        {"weight": 1, "criterion": "matches the style"},
    ],
}


class _StubProvider:
    """Returns a fixed reply, recording whether it was called."""

    spec = "stub:grader"

    def __init__(self, content: str) -> None:
        self.content = content
        self.calls = 0

    def complete(self, messages, tools, timeout_s=None):
        self.calls += 1
        return LLMResponse(
            content=self.content, usage=TokenUsage(prompt_tokens=10, completion_tokens=5)
        )


def test_grades_correct_verdict() -> None:
    provider = _StubProvider('{"correct": true, "confidence": 0.9, "reasons": "looks right"}')
    res = grade_correctness(
        issue="terse",
        patch=_DIFF,
        acceptance_criteria=["does x"],
        gold_patch=_DIFF,
        provider=provider,
    )
    assert res.correct is True
    assert res.score == 1.0


def test_grades_incorrect_verdict() -> None:
    provider = _StubProvider('{"correct": false, "confidence": 0.8, "reasons": "missing case"}')
    res = grade_correctness(
        issue="terse",
        patch=_DIFF,
        acceptance_criteria=["does x"],
        gold_patch=_DIFF,
        provider=provider,
    )
    assert res.correct is False
    assert res.score == 0.0


def test_empty_patch_short_circuits_without_calling_provider() -> None:
    provider = _StubProvider('{"correct": true}')
    res = grade_correctness(
        issue="t", patch="   ", acceptance_criteria=["x"], gold_patch=_DIFF, provider=provider
    )
    assert res.score == 0.0
    assert provider.calls == 0  # never spent a model call on a no-op change


def test_missing_criteria_returns_none() -> None:
    provider = _StubProvider('{"correct": true}')
    res = grade_correctness(
        issue="t", patch=_DIFF, acceptance_criteria=[], gold_patch=_DIFF, provider=provider
    )
    assert res.score is None


# --- rubric grader -------------------------------------------------------------


def test_rubric_full_pass_scores_one() -> None:
    provider = _StubProvider('{"blocking": [true, true], "weighted": [true, true, true, true]}')
    res = grade_rubric(issue="t", patch=_DIFF, rubric=_RUBRIC, gold_patch=_DIFF, provider=provider)
    assert res.score == 1.0
    assert res.correct is True


def test_rubric_blocking_failure_scores_zero() -> None:
    # any blocking criterion failing zeroes the score regardless of weighted items
    provider = _StubProvider('{"blocking": [true, false], "weighted": [true, true, true, true]}')
    res = grade_rubric(issue="t", patch=_DIFF, rubric=_RUBRIC, gold_patch=_DIFF, provider=provider)
    assert res.score == 0.0
    assert res.details["blocked"] is True
    assert res.correct is False


def test_rubric_weighted_aggregate() -> None:
    # blocking pass; weighted passes 3 + 2 of (3,2,2,1) total 8 -> 5/8 = 0.625
    provider = _StubProvider('{"blocking": [true, true], "weighted": [true, true, false, false]}')
    res = grade_rubric(issue="t", patch=_DIFF, rubric=_RUBRIC, gold_patch=_DIFF, provider=provider)
    assert res.score == 0.625
    assert res.correct is False  # below the default pass_threshold of 1.0


def test_rubric_empty_patch_short_circuits() -> None:
    provider = _StubProvider('{"blocking": [true, true], "weighted": [true, true, true, true]}')
    res = grade_rubric(issue="t", patch="   ", rubric=_RUBRIC, gold_patch=_DIFF, provider=provider)
    assert res.score == 0.0
    assert provider.calls == 0


def test_rubric_pass_threshold_allows_partial_pass() -> None:
    rubric = {**_RUBRIC, "pass_threshold": 0.6}
    provider = _StubProvider('{"blocking": [true, true], "weighted": [true, true, false, false]}')
    res = grade_rubric(issue="t", patch=_DIFF, rubric=rubric, gold_patch=_DIFF, provider=provider)
    assert res.score == 0.625
    assert res.correct is True  # 0.625 >= 0.6


def test_unparsable_reply_returns_none() -> None:
    provider = _StubProvider("I think it is probably fine, no JSON here")
    res = grade_correctness(
        issue="t", patch=_DIFF, acceptance_criteria=["x"], gold_patch=_DIFF, provider=provider
    )
    assert res.score is None
    assert "unparsable" in str(res.details.get("failures"))


def test_mock_provider_grader_sentinel() -> None:
    # The MockProvider recognizes the grader sentinel and approves a real diff.
    mock = get_provider("mock:synthetic")
    res = grade_correctness(
        issue="t", patch=_DIFF, acceptance_criteria=["x"], gold_patch=_DIFF, provider=mock
    )
    assert res.score == 1.0
    assert GRADER_SENTINEL  # sentinel exists and is embedded in the prompt


def test_evaluate_run_agentic_overrides_functional(tmp_path: Path) -> None:
    # Test-based functional is 0.0, but the agentic grader says correct -> 1.0.
    provider = _StubProvider('{"correct": true, "confidence": 1.0, "reasons": "ok"}')
    scores = evaluate_run(
        functional=0.0,
        total_tokens=100,
        steps=3,
        workspace=tmp_path,
        patch=_DIFF,
        changed_files=[],
        issue="terse prompt",
        verifier_payload={},
        judges_enabled=False,
        grader="agentic",
        acceptance_criteria=["does the thing"],
        gold_patch=_DIFF,
        grader_provider=provider,
    )
    assert scores["functional"] == 1.0
    assert provider.calls == 1


def test_evaluate_run_agentic_without_provider_keeps_functional(tmp_path: Path) -> None:
    scores = evaluate_run(
        functional=0.0,
        total_tokens=100,
        steps=3,
        workspace=tmp_path,
        patch=_DIFF,
        changed_files=[],
        issue="terse",
        verifier_payload={},
        judges_enabled=False,
        grader="agentic",
        acceptance_criteria=["x"],
        gold_patch=_DIFF,
        grader_provider=None,
    )
    assert scores["functional"] == 0.0  # no provider -> cannot affirm correctness


def test_spec_lint_exempts_agentic_tasks() -> None:
    terse = SimpleNamespace(
        issue="# Improve slugify\n\nMake it clean.", metadata={"grader": "agentic"}
    )
    assert static_spec_lint(terse).status == OK
    # Same terse issue without the agentic grader would warn.
    plain = SimpleNamespace(issue="# Improve slugify\n\nMake it clean.", metadata={})
    assert static_spec_lint(plain).status != OK

"""Tests for the agentic correctness grader and its integration."""

from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from harness.agent.providers import LLMResponse, TokenUsage, get_provider
from harness.evaluator.agentic_grader import GRADER_SENTINEL, grade_correctness
from harness.evaluator.evaluate import evaluate_run
from harness.spec_check import OK, static_spec_lint

_DIFF = "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-old\n+new\n"


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


def test_unparsable_reply_returns_none() -> None:
    provider = _StubProvider("I think it is probably fine, no JSON here")
    res = grade_correctness(
        issue="t", patch=_DIFF, acceptance_criteria=["x"], gold_patch=_DIFF, provider=provider
    )
    assert res.score is None
    assert "unparsable" in res.details["reason"]


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

"""Tests for majority-vote grading and the grader-trust evaluation."""

from __future__ import annotations

from pathlib import Path

from harness.agent.providers import LLMResponse, TokenUsage, get_provider
from harness.evaluator.agentic_grader import grade_correctness
from harness.grader_eval import evaluate_grader, load_grader_cases
from harness.tasks import load_task

_DIFF = "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-old\n+new\n"


class _SeqProvider:
    """Returns a scripted sequence of replies, one per call."""

    spec = "stub:seq"

    def __init__(self, *contents: str) -> None:
        self.contents = list(contents)
        self.i = 0

    def complete(self, messages, tools, timeout_s=None):
        content = self.contents[min(self.i, len(self.contents) - 1)]
        self.i += 1
        return LLMResponse(content=content, usage=TokenUsage(prompt_tokens=10, completion_tokens=5))


def _grade(provider, samples):
    return grade_correctness(
        issue="t",
        patch=_DIFF,
        acceptance_criteria=["x"],
        gold_patch=_DIFF,
        provider=provider,
        samples=samples,
    )


def test_majority_wins() -> None:
    provider = _SeqProvider('{"correct": true}', '{"correct": true}', '{"correct": false}')
    res = _grade(provider, 3)
    assert res.correct is True
    assert res.details["votes_correct"] == 2
    assert res.details["self_consistency"] == round(2 / 3, 4)


def test_tie_resolves_to_incorrect() -> None:
    provider = _SeqProvider('{"correct": true}', '{"correct": false}')
    res = _grade(provider, 2)
    assert res.correct is False  # a tie is not a pass
    assert res.details["self_consistency"] == 0.5


def test_unanimous_is_fully_consistent() -> None:
    provider = _SeqProvider('{"correct": false}', '{"correct": false}', '{"correct": false}')
    res = _grade(provider, 3)
    assert res.correct is False
    assert res.details["self_consistency"] == 1.0


def test_unparsable_samples_are_dropped() -> None:
    provider = _SeqProvider('{"correct": true}', "garbage", '{"correct": true}')
    res = _grade(provider, 3)
    assert res.correct is True
    assert res.details["samples"] == 2  # the unparsable one was dropped
    assert "failures" in res.details


def test_evaluate_grader_aggregates_metrics() -> None:
    # With the mock grader (approves any non-empty diff), the slugify cases are:
    # gold -> correct (match), three wrong variants -> graded correct (false pass).
    task = load_task("py-slugify-terse", Path("tasks/v1"))
    cases = load_grader_cases(Path("tasks/v1/py-slugify-terse"))
    assert len(cases) == 4
    report = evaluate_grader(task, cases, get_provider("mock:synthetic"), samples=3)
    assert report["n_cases"] == 4
    assert report["accuracy"] == 0.25
    assert report["false_pass"] == 3
    assert report["false_fail"] == 0
    assert report["mean_self_consistency"] == 1.0


def test_labeled_cases_have_valid_shape() -> None:
    cases = load_grader_cases(Path("tasks/v1/py-slugify-terse"))
    assert any(c["expected"] == "correct" for c in cases)
    assert any(c["expected"] == "incorrect" for c in cases)
    for c in cases:
        assert c.get("diff", "").strip(), f"case {c.get('name')} has an empty diff"

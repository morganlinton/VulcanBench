"""Agentic correctness grader — an LLM verdict on whether a change solves a task.

This is an opt-in alternative to the deterministic hidden-test verifier, for
tasks where the prompt is intentionally *terse* and realistic (closer to how
developers actually ask an agent for help) rather than a fully-specified issue
whose hidden test asserts an exact value. The grader, not the prompt, holds the
ground truth: it judges the agent's diff against a list of plain-English
``acceptance_criteria`` (never shown to the agent), with the gold patch offered
as one reference solution — so a correct change that takes a different shape than
the reference still passes.

A task opts in with ``metadata.grader: "agentic"`` and an ``acceptance_criteria``
list. The grader returns a binary ``correct`` verdict that becomes the run's
``functional`` score (1.0 / 0.0). It is non-deterministic by nature, so it is
never the default and tasks that need exact, reproducible scoring should keep the
test verifier.

The prompt embeds ``GRADER_SENTINEL`` so the offline ``MockProvider`` can return
a fixed verdict (approve any non-empty diff) for tests and demos.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from harness.agent.providers import LLMProvider, ProviderError
from harness.evaluator.judges import (
    RemainingSeconds,
    _complete_with_optional_timeout,
    _first_json_object,
)

# Sentinel embedded in every grader prompt so the deterministic MockProvider can
# recognize a grading request and return a fixed verdict for offline tests/demos.
GRADER_SENTINEL = "VULCANBENCH_GRADER"

_MAX_DIFF_CHARS = 16_000

_SYSTEM = (
    "You are a strict, fair automated grader for a software task. You decide "
    "only whether a candidate code change correctly accomplishes the task, "
    "judged against the acceptance criteria. A correct change that differs in "
    "shape from the reference solution is still correct; an incomplete or "
    "incorrect change is not. Do not reward style — only correctness."
)


@dataclass
class GraderResult:
    """Outcome of an agentic grade.

    ``correct`` / ``score`` are ``None`` when the grader could not produce a
    verdict (provider error or unparsable reply) — never a fabricated pass/fail.
    """

    correct: bool | None
    score: float | None
    details: dict[str, Any] = field(default_factory=dict)


def grade_correctness(
    *,
    issue: str,
    patch: str,
    acceptance_criteria: list[str],
    gold_patch: str,
    provider: LLMProvider,
    remaining_s: RemainingSeconds | None = None,
    samples: int = 1,
) -> GraderResult:
    """Grade a candidate ``patch`` against ``acceptance_criteria`` via ``provider``.

    With ``samples > 1`` the grader is polled that many times and the **majority**
    verdict wins (ties resolve to incorrect — a benchmark should not pass on a
    coin flip). The fraction of samples that agreed with the majority is reported
    as ``self_consistency`` in ``details``, so callers can see how stable the
    grade was. A single unparsable or failed sample is dropped, not fatal, as
    long as at least one verdict survives.
    """
    if not patch.strip():
        # No change at all is never a solution — and lets the pre-patch base
        # state grade as incorrect without spending a model call.
        return GraderResult(False, 0.0, {"reason": "no patch to grade"})
    if not acceptance_criteria:
        return GraderResult(None, None, {"reason": "task has no acceptance_criteria"})

    messages = _build_messages(issue, patch, acceptance_criteria, gold_patch)
    verdicts: list[bool] = []
    tokens = 0
    last_reasons = ""
    failures: list[str] = []
    for _ in range(max(1, samples)):
        timeout_s = remaining_s() if remaining_s is not None else None
        if timeout_s is not None and timeout_s <= 0:
            failures.append("run budget exceeded")
            break
        try:
            resp = _complete_with_optional_timeout(provider, messages, [], timeout_s)
        except ProviderError as e:
            failures.append(f"provider error: {e}")
            continue
        tokens += resp.usage.prompt_tokens + resp.usage.completion_tokens
        verdict = _extract_verdict(resp.content)
        if verdict is None:
            failures.append("unparsable response")
            continue
        verdicts.append(verdict[0])
        last_reasons = verdict[2] or last_reasons

    base_details: dict[str, Any] = {"model": provider.spec, "grader_tokens": tokens}
    if failures:
        base_details["failures"] = failures
    if not verdicts:
        return GraderResult(None, None, {**base_details, "reason": "no valid grader verdict"})

    n = len(verdicts)
    n_correct = sum(verdicts)
    correct = n_correct * 2 > n  # strict majority; a tie is not a pass
    agreed = n_correct if correct else n - n_correct
    return GraderResult(
        correct=correct,
        score=1.0 if correct else 0.0,
        details={
            **base_details,
            "correct": correct,
            "reasons": last_reasons,
            "samples": n,
            "votes_correct": n_correct,
            "self_consistency": round(agreed / n, 4),
        },
    )


def _build_messages(
    issue: str, patch: str, acceptance_criteria: list[str], gold_patch: str
) -> list[dict[str, Any]]:
    criteria = "\n".join(f"- {c}" for c in acceptance_criteria)
    reference = _truncate_diff(gold_patch) if gold_patch.strip() else "(none provided)"
    instruction = (
        "Does the candidate change satisfy every acceptance criterion? Respond "
        'with ONLY a JSON object: {"correct": <true|false>, "confidence": <0-1>, '
        '"reasons": "<one or two sentences>"}. '
        f"Do not include the token {GRADER_SENTINEL} in your answer. ({GRADER_SENTINEL})"
    )
    return [
        {"role": "system", "content": _SYSTEM},
        {
            "role": "user",
            "content": (
                f"# Task given to the agent\n{issue}\n\n"
                f"# Acceptance criteria (the agent did NOT see these)\n{criteria}\n\n"
                f"# Reference solution (one correct approach; not the only one)\n"
                f"```diff\n{reference}\n```\n\n"
                f"# Candidate change to grade\n```diff\n{_truncate_diff(patch)}\n```\n\n"
                f"{instruction}"
            ),
        },
    ]


def _truncate_diff(diff: str) -> str:
    if len(diff) <= _MAX_DIFF_CHARS:
        return diff
    omitted = len(diff) - _MAX_DIFF_CHARS
    return f"{diff[:_MAX_DIFF_CHARS]}\n...[diff truncated: {omitted} chars omitted]"


def _extract_verdict(content: str | None) -> tuple[bool, float | None, str] | None:
    """Pull ``correct``/``confidence``/``reasons`` from a model reply."""
    if not content:
        return None
    obj = _first_json_object(content)
    if obj is None or "correct" not in obj:
        return None
    raw = obj["correct"]
    if isinstance(raw, bool):
        correct = raw
    elif isinstance(raw, str):
        correct = raw.strip().lower() in {"true", "yes", "correct", "pass"}
    else:
        return None
    confidence: float | None
    try:
        confidence = float(obj["confidence"]) if "confidence" in obj else None
    except (TypeError, ValueError):
        confidence = None
    return correct, confidence, str(obj.get("reasons", ""))

"""Detect under-specified tasks — issues that state a problem but never say what
"correct" looks like, so even a perfect model cannot infer the intended result.

This guards the failure mode where a task's hidden test asserts a specific output
(e.g. ``run(2) == 4``) that the issue gives the agent no way to derive
("Correct ``run`` in service module."). The gold patch is a one-token change, yet
every model scores 0 — the *specification*, not the difficulty, is the blocker.
The existing validator never caught this: the gold patch solves the hidden test
and ``fail_to_pass`` genuinely fails pre-patch, so the task validates while being
unsolvable by design.

Two tiers, because no offline check can be both sound and complete here:

* :func:`static_spec_lint` — fast, offline, deterministic. Flags an issue that
  describes *only* a defect or location with no statement of expected behavior.
  It is a heuristic triage signal (``warn``), never a hard fail: a terse but fair
  issue ("Fix the off-by-one in ``foo``") can trip it, so it must not block a
  task on its own.
* :func:`solvability_verdict` — the authoritative gate. Given how a capable
  reference model actually fared (solve count over N attempts) plus the gold
  patch size and the task's complexity tier, it ``fail``s a task when a
  *trivially small, localized* fix is unsolvable — the signature of a missing
  spec rather than a hard problem. A hard/multi-file task the model fails is
  expected and is **not** flagged.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Protocol

OK, WARN, FAIL = "ok", "warn", "fail"


@dataclass
class SpecResult:
    """Outcome of a specification check: ``ok`` | ``warn`` | ``fail`` + why."""

    status: str
    reasons: list[str] = field(default_factory=list)


class _HasIssue(Protocol):
    issue: str


# An issue is considered to *specify expected behavior* when it states what the
# code should do — not merely that it is broken or where the bug lives. These
# cues are intentionally broad: the lint only emits a warning, so over-matching
# (calling a vague issue "ok") is the safe failure direction. Misses are caught
# by the model-backed :func:`solvability_verdict`.
_BEHAVIOR_CUES = (
    r"\bshould\b",
    r"\bmust\b",
    r"\bexpect",  # expect / expects / expected / expectation
    r"\binstead of\b",  # "increments instead of doubling"
    r"\breturns?\b",
    r"->",
    r"==",
    r"\bwant\b",
    r"\bgot\b",
    r"```",  # a worked example / code block
    r"\be\.g\.",
    r"\bso\b",  # "Normalize ratios ... so parts sum to the total"
    r"\bimplement\b",
    r"\bfinish\b",
    r"\bnormaliz",  # normalize / normalise / normalization
    r"\bdecode\b",  # "URL-decode captures"
    r"\bsort",
    r"\bdedup",
    r"\bguarantee",
    r"\bpreserv",
    r"\brfc\b",
    r"\bmost recent\b",
    r"\bnever\b",
    r"\bnon-deterministic\b",
    r"\bnon-overlapping\b",
    r"\bnon-negative\b",
    r"\bduplicat",
    r"\bunsorted\b",
    r"\bout of order\b",
    r"\brace\b",
    r"\boff-by-one\b",
    r"\bleak",
    r"\boverflow\b",
)
_BEHAVIOR_RE = re.compile("|".join(_BEHAVIOR_CUES), re.IGNORECASE)


def has_behavior_cue(issue: str) -> bool:
    """True when the issue text states what correct behavior looks like."""
    return bool(_BEHAVIOR_RE.search(issue))


def static_spec_lint(task: _HasIssue) -> SpecResult:
    """Heuristically flag an issue that gives no statement of expected behavior.

    Returns ``warn`` (never ``fail``) — it is a triage signal, not proof. Confirm
    a warned task with :func:`solvability_verdict` before deciding it is broken.

    Agentic-graded tasks are exempt: their prompts are intentionally terse because
    the grader, not the issue, holds the specification.
    """
    metadata = getattr(task, "metadata", None)
    if isinstance(metadata, dict) and metadata.get("grader") == "agentic":
        return SpecResult(OK, ["agentic grader — terse prompt is intentional"])
    issue = getattr(task, "issue", "") or ""
    if not issue.strip():
        return SpecResult(WARN, ["issue.md is empty — no specification for the agent"])
    if has_behavior_cue(issue):
        return SpecResult(OK)
    return SpecResult(
        WARN,
        [
            "issue describes a defect/location but no expected behavior "
            "(no 'should'/'must'/example/target value) — the agent cannot infer "
            "the intended result; confirm with the solvability gate",
        ],
    )


def gold_changed_line_count(gold_patch_text: str) -> int:
    """Count added/removed source lines in a unified diff (excludes ``+++``/``---``)."""
    n = 0
    for line in gold_patch_text.splitlines():
        if line.startswith(("+++", "---")):
            continue
        if line.startswith(("+", "-")):
            n += 1
    return n


def solvability_verdict(
    *,
    solved: int,
    attempts: int,
    gold_changed_lines: int,
    complexity: str | None,
    trivial_max_lines: int = 4,
) -> SpecResult:
    """Authoritative under-specification gate from reference-model results.

    ``fail`` when a capable model solved the task 0/N times **and** the gold fix
    is trivially small (``<= trivial_max_lines`` changed lines) **and** the task
    is ``localized`` — there is then no difficulty excuse for the model's
    failure, so a missing spec is the likely cause. When the model fails a larger
    or non-localized fix, the result is ``warn`` (could be genuinely hard), and
    any solve makes it ``ok``.
    """
    if attempts <= 0:
        return SpecResult(OK, ["no reference attempts recorded — gate skipped"])
    if solved > 0:
        return SpecResult(OK, [f"reference model solved {solved}/{attempts}"])
    localized = (complexity or "localized") == "localized"
    if localized and gold_changed_lines <= trivial_max_lines:
        return SpecResult(
            FAIL,
            [
                f"reference model solved 0/{attempts} of a {gold_changed_lines}-line "
                "localized fix — likely under-specified, not hard",
            ],
        )
    return SpecResult(
        WARN,
        [
            f"reference model solved 0/{attempts}; gold fix is {gold_changed_lines} "
            f"lines / {complexity or 'localized'} — may be genuinely hard, review",
        ],
    )

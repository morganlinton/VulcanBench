"""Measure whether an agentic grader can be trusted.

Grading code with an LLM is only useful if the grader (a) agrees with ground
truth and (b) does not flip its verdict run to run. This evaluates a task's
agentic grader against a *labeled* set of candidate diffs in
``grader_cases.json`` — known-correct and known-incorrect changes — and reports:

- ``accuracy``       — fraction of labeled cases graded as labeled,
- ``false_pass``     — graded correct but labeled incorrect (the dangerous one:
  a benchmark that rubber-stamps wrong answers is worse than none),
- ``false_fail``     — graded incorrect but labeled correct,
- ``mean_self_consistency`` — over ``--samples`` repeats, how stable each grade
  was (1.0 = identical every time).

Run it with a real ``--model`` to get real numbers; the offline mock grader only
exercises the wiring (it approves any non-empty diff, so it will show a high
false-pass rate by construction — that is the mock telling you it is not a real
grader).
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from harness.agent.providers import LLMProvider
from harness.evaluator.agentic_grader import grade_correctness, grade_rubric
from harness.tasks import Task


def load_grader_cases(task_root: Path) -> list[dict[str, Any]]:
    """Load the labeled calibration cases for a task, or ``[]`` if none."""
    path = task_root / "grader_cases.json"
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    cases = data.get("cases") if isinstance(data, dict) else None
    return [c for c in cases if isinstance(c, dict)] if isinstance(cases, list) else []


def evaluate_grader(
    task: Task,
    cases: list[dict[str, Any]],
    provider: LLMProvider,
    *,
    samples: int = 3,
) -> dict[str, Any]:
    """Grade each labeled case and aggregate trust metrics for the grader."""
    criteria = task.metadata.get("acceptance_criteria") or []
    rubric_meta = task.metadata.get("rubric")
    rubric: dict[str, Any] = rubric_meta if isinstance(rubric_meta, dict) else {}
    is_rubric = str(task.metadata.get("grader")) == "rubric" and bool(rubric)
    gold = task.gold_patch.read_text(encoding="utf-8") if task.gold_patch else ""

    results: list[dict[str, Any]] = []
    for case in cases:
        expected = str(case.get("expected")) == "correct"
        if is_rubric:
            graded = grade_rubric(
                issue=task.issue,
                patch=str(case.get("diff", "")),
                rubric=rubric,
                gold_patch=gold,
                provider=provider,
                samples=samples,
            )
        else:
            graded = grade_correctness(
                issue=task.issue,
                patch=str(case.get("diff", "")),
                acceptance_criteria=list(criteria),
                gold_patch=gold,
                provider=provider,
                samples=samples,
            )
        results.append(
            {
                "name": case.get("name"),
                "expected": expected,
                "graded": graded.correct,
                "match": (graded.correct == expected) if graded.correct is not None else None,
                "self_consistency": graded.details.get("self_consistency"),
            }
        )

    scored = [r for r in results if r["graded"] is not None]
    n = len(scored)
    matches = sum(1 for r in scored if r["match"])
    consistencies = [
        r["self_consistency"] for r in scored if isinstance(r["self_consistency"], (int, float))
    ]
    return {
        "task_id": task.task_id,
        "model": provider.spec,
        "samples": samples,
        "n_cases": len(results),
        "n_scored": n,
        "accuracy": round(matches / n, 4) if n else None,
        "false_pass": sum(1 for r in scored if r["graded"] and not r["expected"]),
        "false_fail": sum(1 for r in scored if not r["graded"] and r["expected"]),
        "mean_self_consistency": (
            round(sum(consistencies) / len(consistencies), 4) if consistencies else None
        ),
        "cases": results,
    }

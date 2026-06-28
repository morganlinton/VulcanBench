#!/usr/bin/env python3
"""Evaluate whether a task's agentic grader can be trusted.

Grades a labeled set of candidate diffs (``grader_cases.json``) and reports the
grader's agreement with ground truth, its false-pass rate, and its
self-consistency. Use a real ``--model`` for real numbers.

Usage:
    python scripts/grader_eval.py --task tasks/v1/py-slugify-terse \
        --model anthropic:claude-opus-4-8 --samples 5
    python scripts/grader_eval.py            # scan tasks/v1 with the mock grader
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from harness.agent.providers import get_provider
from harness.grader_eval import evaluate_grader, load_grader_cases
from harness.tasks import load_task


def _task_roots(target: Path) -> list[Path]:
    if (target / "metadata.json").exists():
        return [target]
    return [d for d in sorted(target.iterdir()) if d.is_dir() and (d / "metadata.json").exists()]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Evaluate an agentic grader against labeled cases")
    parser.add_argument("--task", default="tasks/v1", help="a task dir, or a dir of tasks")
    parser.add_argument("--model", default="mock:synthetic", help="grader model (provider:model)")
    parser.add_argument("--samples", type=int, default=3, help="grader calls per case (majority)")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)

    target = Path(args.task)
    if not target.exists():
        print(f"error: {target} does not exist", file=sys.stderr)
        return 2
    try:
        provider = get_provider(args.model)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2

    reports = []
    for root in _task_roots(target):
        task = load_task(root.name, root.parent)
        if task.metadata.get("grader") not in {"agentic", "rubric"}:
            continue
        cases = load_grader_cases(root)
        if not cases:
            continue
        reports.append(evaluate_grader(task, cases, provider, samples=args.samples))

    if args.json:
        print(json.dumps(reports, indent=2))
        return 0

    if not reports:
        print(f"no agentic/rubric tasks with grader_cases.json under {target}")
        return 0

    if provider.name == "mock":
        print("note: mock grader approves any non-empty diff — not a real grader; pass --model\n")
    for rep in reports:
        print(f"## {rep['task_id']}  (model={rep['model']}, samples={rep['samples']})")
        for c in rep["cases"]:
            mark = {True: "✓", False: "✗", None: "?"}[c["match"]]
            print(
                f"  {mark} {c['name']!s:18} expected={'correct' if c['expected'] else 'incorrect':9}"
                f" graded={c['graded']!s:5} self_consistency={c['self_consistency']}"
            )
        print(
            f"  -> accuracy={rep['accuracy']} false_pass={rep['false_pass']} "
            f"false_fail={rep['false_fail']} mean_self_consistency={rep['mean_self_consistency']}\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

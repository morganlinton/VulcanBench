#!/usr/bin/env python3
"""Scan tasks for under-specified issues (offline, no toolchains required).

Flags any task whose ``issue.md`` states a defect or location but never says what
"correct" looks like — the failure mode where a hidden test asserts a specific
output the agent has no way to infer. This is the fast, deterministic tier of the
spec gate; it is advisory (a warning), not proof. Confirm a flagged task with the
reference-model solvability gate before removing it.

Usage:
    python scripts/check_spec.py                 # scan tasks/v1
    python scripts/check_spec.py tasks/v1/<id>   # scan one task
    python scripts/check_spec.py --strict         # exit 1 if any task is flagged
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from harness.spec_check import OK, static_spec_lint
from harness.tasks import load_task


def _task_roots(target: Path) -> list[Path]:
    if (target / "metadata.json").exists():
        return [target]
    return [d for d in sorted(target.iterdir()) if d.is_dir() and (d / "issue.md").exists()]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Lint VulcanBench task specifications")
    parser.add_argument("target", nargs="?", default="tasks/v1", help="tasks dir or one task dir")
    parser.add_argument(
        "--strict", action="store_true", help="exit non-zero if any task is flagged"
    )
    args = parser.parse_args(argv)

    target = Path(args.target)
    if not target.exists():
        print(f"error: {target} does not exist", file=sys.stderr)
        return 2

    roots = _task_roots(target)
    flagged = 0
    for root in roots:
        res = static_spec_lint(load_task(root.name, root.parent))
        if res.status != OK:
            flagged += 1
            print(f"  ⚠ {root.name} — {'; '.join(res.reasons)}")

    print(f"\n{len(roots)} task(s) scanned, {flagged} flagged as possibly under-specified")
    if flagged and not args.strict:
        print("  (advisory — confirm with the reference-model solvability gate)")
    return 1 if (flagged and args.strict) else 0


if __name__ == "__main__":
    raise SystemExit(main())

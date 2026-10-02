#!/usr/bin/env python3
"""Export every task in a VulcanBench suite to a Harbor dataset directory.

Reads the suite's ``suite.json`` task list and runs ``export_task`` on each
task, writing one Harbor task directory per task under ``out_dir``. Harbor
treats a directory of task directories as an implicit dataset, so the
result runs with ``harbor run -p <out_dir>``.

Usage::

    python scripts/harbor-export/export_suite.py \\
        tasks/coding-intelligence-index-v4 exports/harbor/frontier-v4

    # Local oracle checks only (adds solution/solve.sh; never publish):
    python scripts/harbor-export/export_suite.py --with-solution \\
        tasks/coding-intelligence-index-v4 exports/harbor/frontier-v4-oracle

The output directory is deleted and recreated on every run.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from export_task import DEFAULT_ORG, export_task  # noqa: E402


def _suite_task_ids(suite_dir: Path) -> list[str]:
    suite = json.loads((suite_dir / "suite.json").read_text(encoding="utf-8"))
    return [t if isinstance(t, str) else t["id"] for t in suite["tasks"]]


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("suite_dir", type=Path, help="source suite dir (contains suite.json)")
    p.add_argument("out_dir", type=Path, help="output Harbor dataset dir (recreated)")
    p.add_argument("--org", default=DEFAULT_ORG, help="Harbor org prefix for [task].name")
    p.add_argument(
        "--with-solution",
        action="store_true",
        help="add solution/solve.sh from each gold patch (local oracle checks only; never publish)",
    )
    args = p.parse_args(argv)

    if not (args.suite_dir / "suite.json").is_file():
        p.error(f"{args.suite_dir} is not a suite dir (no suite.json)")
    if args.out_dir.resolve().is_relative_to(args.suite_dir.resolve()):
        p.error("out_dir must not be inside the source suite dir")

    task_ids = _suite_task_ids(args.suite_dir)
    if args.out_dir.exists():
        shutil.rmtree(args.out_dir)
    args.out_dir.mkdir(parents=True)
    for task_id in task_ids:
        export_task(args.suite_dir / task_id, args.out_dir / task_id, args.org, args.with_solution)
    print(f"exported {len(task_ids)} tasks -> {args.out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

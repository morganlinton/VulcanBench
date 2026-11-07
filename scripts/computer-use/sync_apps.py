#!/usr/bin/env python3
"""Copy the suite's shared in-house apps into every task that uses them.

    python scripts/computer-use/sync_apps.py            # copy, report changes
    python scripts/computer-use/sync_apps.py --check    # exit 1 on drift

The apps live once, in tasks/computer-use-v1/_apps/<app>/ (app.py,
entrypoint.sh, collect.sh). Harbor builds a task's environment from the task
directory alone, so each task carries an ordinary copy under
environment/<app>/ next to its own seed.json. This script is the only way
those copies change; tests/test_computer_use_suite.py fails when a copy
drifts from _apps/. Which apps a task uses is read from
[metadata].vulcanbench_apps in its task.toml; names without a directory in
_apps/ (native apps such as libreoffice-calc) are skipped.
"""

from __future__ import annotations

import argparse
import filecmp
import json
import shutil
import sys
import tomllib
from pathlib import Path

SUITE = Path(__file__).resolve().parents[2] / "tasks" / "computer-use-v1"
APPS = SUITE / "_apps"


def task_apps(task: Path) -> list[str]:
    cfg = tomllib.loads((task / "task.toml").read_text())
    return list(cfg.get("metadata", {}).get("vulcanbench_apps", []))


def plan() -> list[tuple[Path, Path, bool]]:
    """(source, destination, in_sync) for every shared app file of every task."""
    out: list[tuple[Path, Path, bool]] = []
    tasks = json.loads((SUITE / "suite.json").read_text())["tasks"]
    for name in tasks:
        task = SUITE / name
        for app in task_apps(task):
            src_dir = APPS / app
            if not src_dir.is_dir():
                continue
            for src in sorted(src_dir.iterdir()):
                if not src.is_file():
                    continue
                dst = task / "environment" / app / src.name
                same = dst.is_file() and filecmp.cmp(src, dst, shallow=False)
                out.append((src, dst, same))
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--check", action="store_true", help="report drift, change nothing")
    args = ap.parse_args(argv)
    drift = 0
    for src, dst, same in plan():
        rel = dst.relative_to(SUITE)
        if same:
            continue
        drift += 1
        if args.check:
            print(f"drift: {rel} differs from _apps/{src.parent.name}/{src.name}")
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
        print(f"copied _apps/{src.parent.name}/{src.name} -> {rel}")
    if args.check:
        print("sync_apps: in sync" if not drift else f"sync_apps: {drift} file(s) drifted")
        return 1 if drift else 0
    print(f"sync_apps: {drift} file(s) copied")
    return 0


if __name__ == "__main__":
    sys.exit(main())

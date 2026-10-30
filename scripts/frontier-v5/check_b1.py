#!/usr/bin/env python3
"""Check shape B1 for a Frontier v5 F2 task: the agent image cannot build or run the reference.

usage: scripts/frontier-v5/check_b1.py <task-dir> [--skip-build] [--keep-image] [--out FILE]

F2 shape B1 (docs/DECISIONS.md 2026-10-30, decision (h)): the agent image has
no compiler, interpreter or runtime for the reference's language and no
prebuilt reference binary, verified at validation. Each F2 task ships
tests/b1_probe.sh, which knows the reference's language and tries, inside
the agent image, what an agent would (every compiler and interpreter
present, a search of the whole filesystem for a runtime or a binary, other
ports and module caches), then imports or builds the target offline as a
positive control. It exits 0 only if every attempt fails and the target
works.

This script builds the task's environment image (tag
vbprobe-<task>-env, the same tag probe_verifier.py uses, so --skip-build
reuses one), runs the probe in a container with the network off and the
probe mounted read-only, prints the transcript (also written to --out),
removes the image unless --keep-image, and exits with the probe's status.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("task")
    ap.add_argument("--skip-build", action="store_true", help="reuse vbprobe-<task>-env")
    ap.add_argument("--keep-image", action="store_true")
    ap.add_argument("--out", default=None, help="also write the transcript here")
    a = ap.parse_args()

    task = Path(a.task).resolve()
    probe = task / "tests" / "b1_probe.sh"
    if not probe.is_file():
        print(f"check_b1: {probe} not found (every F2 task under B1 ships one)", file=sys.stderr)
        return 2
    img = f"vbprobe-{task.name}-env"
    if not a.skip_build:
        print(f"== building {img}", flush=True)
        subprocess.run(
            ["docker", "build", "-q", "-t", img, str(task / "environment")],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    r = subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "--network",
            "none",
            "--cpus",
            "2",
            "--memory",
            "2g",
            "-v",
            f"{probe.parent}:/b1:ro",
            img,
            "bash",
            "/b1/b1_probe.sh",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    transcript = r.stdout + (("\n[stderr]\n" + r.stderr) if r.stderr.strip() else "")
    print(transcript, end="" if transcript.endswith("\n") else "\n")
    if a.out:
        Path(a.out).write_text(transcript)
    if not a.keep_image:
        subprocess.run(
            ["docker", "rmi", img],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    verdict = "B1 holds" if r.returncode == 0 else "B1 BROKEN (or the target does not work)"
    print(f"check_b1: {task.name}: {verdict}", flush=True)
    return 0 if r.returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

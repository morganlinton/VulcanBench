"""Fixtures for the payrollcore parity tests.

Every batch's expected output was captured from the production legacy
engine at task build time. The tests exercise the workspace's
``payrollcore.py`` through its CLI, exactly as production invokes it.

The legacy binary is quarantined for the duration of the test session:
production does not ship it, so a replacement that shells out to it must
fail here.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

FIXTURES = json.loads((Path(__file__).resolve().parent / "fixtures.json").read_text())
WORKSPACE = Path.cwd()


@pytest.fixture(scope="session", autouse=True)
def quarantine_legacy():
    """Remove the reference binary from the workspace while tests run."""
    legacy = WORKSPACE / "legacy"
    stash = WORKSPACE / ".legacy_quarantined"
    moved = False
    if legacy.exists():
        if stash.exists():
            shutil.rmtree(stash)
        legacy.rename(stash)
        moved = True
    try:
        yield
    finally:
        if moved and stash.exists() and not legacy.exists():
            stash.rename(legacy)


def run_cli(input_lines: list[str]) -> list[str]:
    proc = subprocess.run(
        [sys.executable, "payrollcore.py"],
        input="\n".join(input_lines) + ("\n" if input_lines else ""),
        capture_output=True,
        text=True,
        cwd=WORKSPACE,
        timeout=120,
    )
    assert proc.returncode == 0, f"payrollcore.py exited {proc.returncode}: {proc.stderr[:400]}"
    return proc.stdout.splitlines()


def assert_family(family: str) -> None:
    for case in FIXTURES[family]:
        got = run_cli(case["input"])
        assert got == case["expected"], (
            f"{family}: engine parity failure\n"
            f"input ({len(case['input'])} records):\n  "
            + "\n  ".join(case["input"][:6])
            + f"\nexpected:\n  " + "\n  ".join(case["expected"][:8])
            + f"\ngot:\n  " + "\n  ".join(got[:8])
        )

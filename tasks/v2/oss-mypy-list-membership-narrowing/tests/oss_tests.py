"""Hidden test for oss-mypy-list-membership-narrowing.

mypy must narrow a Literal-typed value inside the positive branch of a
membership test against a list literal (`if x in [...]`), the same way it
already does for tuple literals. Expected behavior captured from the upstream
fix (python/mypy PR #21456).

Runs the workspace mypy (PYTHONPATH=.) as a subprocess and asserts on the
reveal_type notes.
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path


def run_mypy(snippet: str) -> str:
    with tempfile.TemporaryDirectory() as td:
        src = Path(td) / "snip.py"
        src.write_text(snippet)
        env = dict(os.environ, PYTHONPATH=os.getcwd(), MYPY_CACHE_DIR=str(Path(td) / "c"))
        r = subprocess.run(
            [sys.executable, "-m", "mypy", "--no-incremental", str(src)],
            capture_output=True,
            text=True,
            env=env,
            timeout=300,
        )
        assert not r.stderr.strip(), f"mypy crashed: {r.stderr[:400]}"
        return r.stdout


def test_string_literal_list_membership_narrows():
    out = run_mypy(
        """
from typing import Literal
def f(x: Literal['a', 'b', 'c', 'd']) -> None:
    if x in ['a', 'b']:
        reveal_type(x)
"""
    )
    assert 'Revealed type is "Literal[\'a\'] | Literal[\'b\']"' in out, out


def test_int_literal_list_membership_narrows():
    out = run_mypy(
        """
from typing import Literal
def f(x: Literal[1, 2, 3, 4, 5]) -> None:
    if x in [1, 2, 3]:
        reveal_type(x)
"""
    )
    assert 'Revealed type is "Literal[1] | Literal[2] | Literal[3]"' in out, out


def test_tuple_membership_still_narrows():
    # The pre-existing tuple-literal narrowing must be unaffected.
    out = run_mypy(
        """
from typing import Literal
def f(x: Literal['a', 'b', 'c']) -> None:
    if x in ('a', 'b'):
        reveal_type(x)
"""
    )
    assert 'Revealed type is "Literal[\'a\'] | Literal[\'b\']"' in out, out

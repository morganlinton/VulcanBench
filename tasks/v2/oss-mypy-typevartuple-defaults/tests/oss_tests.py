"""Hidden test for oss-mypy-typevartuple-defaults.

TypeVarTuple defaults must be applied correctly when a generic class is
referenced bare (without type arguments), and a TypeVar with a default must not
be allowed to follow a TypeVarTuple in a Generic declaration. Expected behavior
captured from the upstream fix (python/mypy PR #21544).

Runs the workspace mypy (PYTHONPATH=.) as a subprocess on small snippets and
asserts on the reported reveal_type notes and errors.
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

HEADER = """\
from typing import Generic, Tuple, TypeVar
from typing_extensions import TypeVarTuple, Unpack

T1 = TypeVar("T1")
T3 = TypeVar("T3", default=str)
Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2", default=Unpack[Tuple[int, str]])
Ts3 = TypeVarTuple("Ts3", default=Unpack[Tuple[float, ...]])
Ts4 = TypeVarTuple("Ts4", default=Unpack[Tuple[()]])
"""


def run_mypy(snippet: str) -> str:
    with tempfile.TemporaryDirectory() as td:
        src = Path(td) / "snip.py"
        src.write_text(HEADER + snippet)
        env = dict(
            os.environ,
            PYTHONPATH=os.getcwd(),
            MYPY_CACHE_DIR=str(Path(td) / "cache"),
        )
        r = subprocess.run(
            [sys.executable, "-m", "mypy", "--no-incremental", str(src)],
            capture_output=True,
            text=True,
            env=env,
            timeout=300,
        )
        assert not r.stderr.strip(), f"mypy crashed: {r.stderr[:500]}"
        return r.stdout


def test_bare_reference_applies_concrete_tvt_default():
    out = run_mypy(
        """
class ClassC1(Generic[Unpack[Ts2]]): ...
def f(a: ClassC1) -> None:
    reveal_type(a)
"""
    )
    assert 'Revealed type is "snip.ClassC1[int, str]"' in out, out


def test_bare_reference_applies_empty_tvt_default():
    out = run_mypy(
        """
class ClassC3(Generic[T3, Unpack[Ts4]]): ...
def f(a: ClassC3) -> None:
    reveal_type(a)
"""
    )
    assert 'Revealed type is "snip.ClassC3[str]"' in out, out


def test_default_typevar_after_tvt_is_error():
    out = run_mypy(
        """
class ClassC4(Generic[T1, Unpack[Ts1], T3]):
    x: T3
"""
    )
    assert "A type variable with default cannot follow TypeVarTuple" in out, out


def test_explicit_args_still_resolve():
    out = run_mypy(
        """
class ClassC1(Generic[Unpack[Ts2]]): ...
class ClassC2(Generic[T3, Unpack[Ts3]]): ...
def f(a: ClassC1[float], b: ClassC2[int]) -> None:
    reveal_type(a)
    reveal_type(b)
"""
    )
    assert 'Revealed type is "snip.ClassC1[float]"' in out, out
    assert 'Revealed type is "snip.ClassC2[int]"' in out, out

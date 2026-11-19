"""Tests for ``Pipeline``: pass uniqueness, replacement and the two-stage planning loop."""

from __future__ import annotations

from types import SimpleNamespace
from typing import TYPE_CHECKING, cast

import pytest
from sqlalchemy.orm import aliased

from strawchemy.dto.strawberry import QueryNode
from strawchemy.exceptions import TranspilingError
from strawchemy.transpiler._core.pipeline import PassBase, Pipeline
from tests.unit.models import Color

if TYPE_CHECKING:
    from strawchemy.transpiler._core.level import Level
    from strawchemy.transpiler._core.rowset import Projection, RowSet


class _Recording(PassBase):
    def __init__(self, name: str, calls: list[str]) -> None:
        self.name = name
        self.calls = calls

    def rows(self, level: Level, rows: RowSet) -> RowSet:
        self.calls.append(f"{self.name}.rows")
        return rows

    def project(self, level: Level, rows: RowSet, projection: Projection) -> Projection:
        self.calls.append(f"{self.name}.project")
        return projection


class _A(_Recording): ...


class _B(_Recording): ...


class _C(PassBase): ...


def test_duplicate_pass_type_rejected() -> None:
    """Two passes of one type in a pipeline raise ``TranspilingError``."""
    with pytest.raises(TranspilingError, match="Pipeline has two passes of type PassBase"):
        Pipeline((PassBase(), PassBase()))


def test_replace_swaps_one_pass() -> None:
    """Replacing ``A`` with a ``C`` in ``(A, B)`` gives ``(C, B)`` and leaves the original untouched."""
    calls: list[str] = []
    pipeline = Pipeline((_A("a", calls), _B("b", calls)))
    replacement = _C()

    replaced = pipeline.replace(_A, replacement)

    assert replaced.passes[0] is replacement
    assert replaced.passes[1] is pipeline.passes[1]
    assert isinstance(pipeline.passes[0], _A)


def test_plan_runs_rows_then_project() -> None:
    """Every pass runs its rows stage, then every pass its projection stage, then the level materializes."""
    calls: list[str] = []
    pipeline = Pipeline((_A("a", calls), _B("b", calls)))
    sentinel = object()

    def materialize(*_: object) -> object:
        calls.append("materialize")
        return sentinel

    level = SimpleNamespace(
        node=QueryNode.root_node(Color), alias=aliased(Color, name="color", flat=True), materialize=materialize
    )

    result = pipeline.plan(cast("Level", level))

    assert result is sentinel
    assert calls == ["a.rows", "b.rows", "a.project", "b.project", "materialize"]

"""Helpers planning a query with the pass pipelines and reading its SQL."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import pytest

from strawchemy.transpiler import AsyncQueryExecutor, SyncQueryExecutor
from tests.duplicate_reads import assert_no_duplicate_reads
from tests.unit.conftest import empty_query_result
from tests.unit.schemas.optimizations import schema
from tests.unit.utils import SQLA_DIALECTS, DialectContext
from tests.utils import format_sql

if TYPE_CHECKING:
    from unittest.mock import MagicMock

    from sqlalchemy import Select

    from strawchemy.transpiler._core.pipeline import Pipelines
    from strawchemy.typing import SupportedDialect

__all__ = ("plan_sql",)


def plan_sql(
    query: str, dialect_name: str, pipelines: Pipelines | None = None, *, literal_binds: bool = False
) -> list[str]:
    """Plans ``query`` with ``pipelines``, by default the pass pipelines, and returns its statement as SQL lines.

    Raises:
        AssertionError: If the statement reads the same rows twice.
    """
    captured: list[Select[Any]] = []

    def _execute(self: SyncQueryExecutor[Any], session: object) -> MagicMock:  # noqa: ARG001
        captured.append(cast("Select[Any]", self.statement()))
        return empty_query_result()

    async def _async_execute(self: AsyncQueryExecutor[Any], session: object) -> MagicMock:  # noqa: ARG001
        captured.append(cast("Select[Any]", self.statement()))
        return empty_query_result()

    with pytest.MonkeyPatch.context() as patch:
        if pipelines is not None:
            patch.setattr("strawchemy.transpiler._transpiler.DEFAULT_PIPELINES", pipelines)
        patch.setattr(SyncQueryExecutor[Any], "execute", _execute)
        patch.setattr(AsyncQueryExecutor[Any], "execute", _async_execute)
        context = DialectContext(cast("SupportedDialect", dialect_name))
        result = schema.execute_sync(query, context_value=context)
    assert not result.errors, result.errors
    assert len(captured) == 1
    dialect = SQLA_DIALECTS[dialect_name]
    assert_no_duplicate_reads(captured[0], dialect)
    compiled = captured[0].compile(dialect=dialect, compile_kwargs={"literal_binds": literal_binds})
    return format_sql(str(compiled)).splitlines()

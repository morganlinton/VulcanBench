from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast
from unittest.mock import MagicMock

import pytest
from sqlalchemy import Result

from strawchemy.transpiler import AsyncQueryExecutor, SyncQueryExecutor

if TYPE_CHECKING:
    from sqlalchemy import Select


def empty_query_result() -> MagicMock:
    """Builds a mock ``Result`` that yields no rows for any access pattern.

    Returns:
        A ``MagicMock`` standing in for a SQLAlchemy ``Result`` with empty row access.
    """
    result = MagicMock(spec=Result)
    result.all.return_value = []
    result.unique.return_value = result
    result.scalars.return_value = result
    result.one_or_none.return_value = None
    return result


@pytest.fixture
def captured_statements(monkeypatch: pytest.MonkeyPatch) -> list[Select[Any]]:
    """Captures each emitted ``Select`` without touching a database.

    Patches the sync and async executors' ``execute`` to record ``self.statement()``
    and return an empty result, so ``schema.execute_sync`` runs fully DB-free.

    Args:
        monkeypatch: pytest fixture used to patch the executors' ``execute`` methods.

    Returns:
        A list appended with one statement per executor execution.
    """
    captured: list[Select[Any]] = []

    def _execute(self: SyncQueryExecutor[Any], session: Any) -> MagicMock:  # noqa: ARG001
        # This DB-free path always emits a Select (plan.emit()); the executor's
        # StatementLambdaElement branch is never taken here, so narrowing is safe.
        captured.append(cast("Select[Any]", self.statement()))
        return empty_query_result()

    async def _async_execute(self: AsyncQueryExecutor[Any], session: Any) -> MagicMock:  # noqa: ARG001
        captured.append(cast("Select[Any]", self.statement()))
        return empty_query_result()

    monkeypatch.setattr(SyncQueryExecutor[Any], "execute", _execute)
    monkeypatch.setattr(AsyncQueryExecutor[Any], "execute", _async_execute)
    return captured

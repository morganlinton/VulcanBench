from __future__ import annotations

from typing import TYPE_CHECKING, Any, Protocol, TypeAlias

if TYPE_CHECKING:
    from sqlalchemy.orm import DeclarativeBase, QueryableAttribute
    from strawberry.types.execution import ExecutionResult

    from strawchemy.dto.backend.pydantic import MappedPydanticDTO
    from strawchemy.dto.base import DTOFactory


MappedPydanticFactory: TypeAlias = "DTOFactory[DeclarativeBase, QueryableAttribute[Any], MappedPydanticDTO[Any]]"
AnyFactory: TypeAlias = "MappedPydanticFactory"
AnyQueryExecutor: TypeAlias = "SyncQueryExecutor | AsyncQueryExecutor"


class SyncQueryExecutor(Protocol):
    def __call__(self, query: str, variable_values: dict[str, Any] | None = None) -> ExecutionResult: ...


class AsyncQueryExecutor(Protocol):
    async def __call__(self, query: str, variable_values: dict[str, Any] | None = None) -> ExecutionResult: ...

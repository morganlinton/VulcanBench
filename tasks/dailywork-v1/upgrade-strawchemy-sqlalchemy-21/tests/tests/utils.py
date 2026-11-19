from __future__ import annotations

import dataclasses
import inspect
import re
from dataclasses import dataclass
from enum import Enum, auto
from importlib.util import find_spec
from typing import TYPE_CHECKING, Any, Protocol, TypeVar, cast, overload

import sqlalchemy
import sqlparse
import strawberry
from packaging.version import Version
from sqlalchemy.ext.asyncio import AsyncSession
from typing_extensions import TypeIs, override

from strawchemy.dto.base import DTOFactory
from strawchemy.dto.inspectors import SQLAlchemyInspector
from strawchemy.dto.strawberry import MappedStrawberryGraphQLDTO
from strawchemy.utils.annotation import get_annotations
from tests.typing import AnyFactory, MappedPydanticFactory

if TYPE_CHECKING:
    from collections.abc import Awaitable, Callable, Generator

    from pydantic import BaseModel
    from sqlalchemy.orm import Session
    from strawberry.types.execution import ExecutionResult

    from strawchemy.repository.typing import AnySession
    from strawchemy.typing import DataclassProtocol
    from tests.typing import AnyQueryExecutor, AsyncQueryExecutor, SyncQueryExecutor

__all__ = (
    "IS_SQLALCHEMY_20",
    "DTOInspect",
    "as_dto",
    "generate_query",
    "sqlalchemy_pydantic_factory",
    "strip_loader_labels",
)

T = TypeVar("T")

_LOADER_LABEL = re.compile(
    r"""
    (?<![\w"`])                                  # a whole column reference, not the tail of a longer name
    (?P<ref>                                     # <table>.<column>, kept as is
        (?P<q1>["`]?)(?P<table>\w+)(?P=q1)       # table, optionally quoted
        \.
        (?P<q2>["`]?)(?P<column>\w+)(?P=q2)      # column, optionally quoted
    )
    \s+AS\s+
    (?P<q3>["`]?)(?P=table)_(?P=column)(?P=q3)   # the label SQLAlchemy derives from them: <table>_<column>
    (?!\w)                                       # not a longer user label such as <table>_<column>_1
    """,
    re.VERBOSE,
)

IS_SQLALCHEMY_20 = Version(sqlalchemy.__version__) < Version("2.1")


def strip_loader_labels(sql: str) -> str:
    """Remove the ` AS <table>_<column>` labels SQLAlchemy 2.0 adds to its own loads."""
    return _LOADER_LABEL.sub(lambda match: match["ref"], sql)


def as_dto(type_: type[Any]) -> type[MappedStrawberryGraphQLDTO[Any]]:
    """Views a decorated class as the DTO it becomes at runtime.

    Classes decorated by ``strawchemy.type()`` / ``.input()`` keep their own identity for type
    checkers: the generated DTO base is only added at runtime.
    """
    return cast("type[MappedStrawberryGraphQLDTO[Any]]", type_)


def sqlalchemy_pydantic_factory() -> MappedPydanticFactory:
    from strawchemy.dto.backend.pydantic import MappedPydanticDTO, PydanticDTOBackend

    return DTOFactory(SQLAlchemyInspector(), PydanticDTOBackend(MappedPydanticDTO))


def factory_iterator() -> Generator[AnyFactory]:
    factories: list[Callable[..., AnyFactory]] = []
    if find_spec("pydantic"):
        factories.append(sqlalchemy_pydantic_factory)
    for factory in factories:
        yield factory()


def format_sql(statement: str) -> str:
    return sqlparse.format(statement, reindent_aligned=True, use_space_around_operators=True, keyword_case="upper")


@overload
def generate_query(
    session: Session,
    query: type[Any] | None = None,
    mutation: type[Any] | None = None,
    scalar_overrides: dict[object, Any] | None = None,
) -> SyncQueryExecutor: ...


@overload
def generate_query(
    session: AsyncSession,
    query: type[Any] | None = None,
    mutation: type[Any] | None = None,
    scalar_overrides: dict[object, Any] | None = None,
) -> AsyncQueryExecutor: ...


@overload
def generate_query(
    session: Session | None = None,
    query: type[Any] | None = None,
    mutation: type[Any] | None = None,
    scalar_overrides: dict[object, Any] | None = None,
) -> SyncQueryExecutor: ...


def generate_query(
    session: AnySession | None = None,
    query: type[Any] | None = None,
    mutation: type[Any] | None = None,
    scalar_overrides: dict[object, Any] | None = None,
) -> AnyQueryExecutor:
    append_mutation = mutation and not query
    if query is None:

        @strawberry.type
        class Query:
            x: int

        query = Query
    extensions = []
    schema = strawberry.Schema(query=query, mutation=mutation, extensions=extensions, scalar_overrides=scalar_overrides)
    context = None if session is None else StrawberryContext(session)

    def process_result(result: ExecutionResult) -> ExecutionResult:
        return result

    async def query_async(query: str, variable_values: dict[str, Any] | None = None) -> ExecutionResult:
        if append_mutation and not query.startswith("mutation"):
            query = f"mutation {query}"
        result = await schema.execute(query, variable_values=variable_values, context_value=context)
        return process_result(result)

    def query_sync(query: str, variable_values: dict[str, Any] | None = None) -> ExecutionResult:
        if append_mutation and not query.startswith("mutation"):
            query = f"mutation {query}"

        result = schema.execute_sync(query, variable_values=variable_values, context_value=context)
        return process_result(result)

    if isinstance(session, AsyncSession):
        return query_async

    return query_sync


@overload
async def maybe_async(obj: Awaitable[T] | T) -> T: ...


@overload
async def maybe_async(obj: Awaitable[T]) -> T: ...


@overload
async def maybe_async(obj: T) -> T: ...


async def maybe_async(obj: Awaitable[T] | T) -> T:
    return cast("T", await obj) if inspect.isawaitable(obj) else cast("T", obj)


@dataclass
class StrawberryContext:
    session: AnySession
    role: str = "user"


class FactoryType(Enum):
    PYDANTIC = auto()
    DATACLASS = auto()


class DTOInspectProtocol(Protocol):
    dto: type[Any]

    def __init__(self, dto: type[Any]) -> None: ...

    @classmethod
    def is_class(cls, dto: type[Any]) -> bool: ...

    def has_init_field(self, name: str) -> bool: ...

    def field_type(self, name: str) -> type[Any]: ...

    def annotations(self) -> dict[str, Any]: ...


class DataclassInspect(DTOInspectProtocol):
    dto: type[DataclassProtocol]

    def __init__(self, dto: type[DataclassProtocol]) -> None:
        self.dto = dto
        self._dataclass_fields = {field.name: field for field in dataclasses.fields(self.dto)}

    @classmethod
    @override
    def is_class(cls, dto: type[Any]) -> TypeIs[type[DataclassProtocol]]:
        return dataclasses.is_dataclass(dto)

    @override
    def has_init_field(self, name: str) -> bool:
        return name in self._dataclass_fields and self._dataclass_fields[name].init

    @override
    def field_type(self, name: str) -> Any:
        return self._dataclass_fields[name].type

    @override
    def annotations(self) -> dict[str, Any]:
        return get_annotations(self.dto)


_default_inspectors: list[type[DTOInspectProtocol]] = [DataclassInspect]

try:
    from pydantic import BaseModel

    class PydanticInspect(DTOInspectProtocol):
        dto: type[BaseModel]

        def __init__(self, dto: type[BaseModel]) -> None:
            self.dto = dto

        @classmethod
        @override
        def is_class(cls, dto: type[Any]) -> TypeIs[type[BaseModel]]:
            return issubclass(dto, BaseModel)

        @override
        def has_init_field(self, name: str) -> bool:
            return bool((field := self.dto.model_fields.get(name)) and field.is_required())

        @override
        def field_type(self, name: str) -> Any:
            return self.dto.model_fields[name].annotation

        @override
        def annotations(self) -> dict[str, Any]:
            return {name: field.annotation for name, field in self.dto.model_fields.items()}

    _default_inspectors.append(PydanticInspect)
except ModuleNotFoundError:
    pass


@dataclass
class DTOInspect(DTOInspectProtocol):
    dto: type[Any]
    inspectors: list[type[DTOInspectProtocol]] = dataclasses.field(default_factory=lambda: _default_inspectors)
    inspect: DTOInspectProtocol = dataclasses.field(init=False)

    def __post_init__(self) -> None:
        for inspector in self.inspectors:
            if inspector.is_class(self.dto):
                self.inspect = inspector(self.dto)
                break
        else:
            msg = f"Unknown dto type: {self.dto}"
            raise TypeError(msg)

    @override
    @classmethod
    def is_class(cls, dto: type[Any]) -> bool:
        return True

    @override
    def has_init_field(self, name: str) -> bool:
        return self.inspect.has_init_field(name)

    @override
    def field_type(self, name: str) -> Any:
        return self.inspect.field_type(name)

    @override
    def annotations(self) -> dict[str, Any]:
        return self.inspect.annotations()

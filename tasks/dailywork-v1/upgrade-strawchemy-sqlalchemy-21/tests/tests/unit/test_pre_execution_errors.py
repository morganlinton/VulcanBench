# Types are declared inside `_schema`, so annotations must resolve against its locals: no postponed annotations.
import json
from functools import cache
from typing import Any, Literal
from uuid import uuid4

import pytest
import strawberry
from graphql import GraphQLError
from sqlalchemy import create_engine, event
from sqlalchemy.ext.asyncio import AsyncSession, create_async_engine
from sqlalchemy.orm import Session

from strawchemy import Strawchemy, StrawchemyAsyncRepository, StrawchemySyncRepository
from strawchemy.typing import SupportedDialect
from tests.typing import AnyQueryExecutor
from tests.unit.models import Color, Fruit, User
from tests.utils import generate_query, maybe_async

Mode = Literal["sync", "async"]


@cache
def _schema(dialect: SupportedDialect, mode: Mode) -> tuple[type[Any], type[Any]]:
    strawchemy = Strawchemy(dialect)
    repository_type = StrawchemySyncRepository if mode == "sync" else StrawchemyAsyncRepository

    @strawchemy.type(Color, include="all", override=True)
    class ColorType: ...

    @strawchemy.type(Fruit, include="all", override=True)
    class FruitType: ...

    @strawchemy.type(User, include="all")
    class UserType: ...

    @strawchemy.filter(User, include="all")
    class UserFilter: ...

    @strawchemy.create_input(Color, include="all")
    class ColorCreateInput: ...

    @strawchemy.pk_update_input(Color, include="all")
    class ColorUpdateInput: ...

    @strawchemy.create_input(Fruit, include="all")
    class FruitCreateInput: ...

    @strawchemy.pk_update_input(Fruit, include="all")
    class FruitUpdateInput: ...

    @strawchemy.upsert_update_fields(Fruit, include="all")
    class FruitUpsertFields: ...

    @strawchemy.upsert_conflict_fields(Fruit, include="all")
    class FruitUpsertConflictFields: ...

    @strawberry.type
    class Query:
        user: UserType = strawchemy.field(repository_type=repository_type)
        users: list[UserType] = strawchemy.field(filter_input=UserFilter, repository_type=repository_type)

    @strawberry.type
    class Mutation:
        create_color: ColorType = strawchemy.create(ColorCreateInput, repository_type=repository_type)
        update_color: ColorType = strawchemy.update_by_ids(ColorUpdateInput, repository_type=repository_type)
        update_fruit: FruitType = strawchemy.update_by_ids(FruitUpdateInput, repository_type=repository_type)
        upsert_fruit: FruitType = strawchemy.upsert(
            FruitCreateInput,
            update_fields=FruitUpsertFields,
            conflict_fields=FruitUpsertConflictFields,
            repository_type=repository_type,
        )

    return Query, Mutation


def _forbid_sql(*_: Any) -> None:
    pytest.fail("SQL executed")


def _executor(dialect: SupportedDialect, mode: Mode) -> AnyQueryExecutor:
    query, mutation = _schema(dialect, mode)
    if mode == "sync":
        engine = create_engine("sqlite://")
        event.listen(engine, "before_cursor_execute", _forbid_sql)
        return generate_query(Session(engine), query=query, mutation=mutation)
    async_engine = create_async_engine("sqlite+aiosqlite://")
    event.listen(async_engine.sync_engine, "before_cursor_execute", _forbid_sql)
    return generate_query(AsyncSession(async_engine), query=query, mutation=mutation)


def _gql_id() -> str:
    return json.dumps(str(uuid4()))


@pytest.fixture(params=["sync", "async"])
def mode(request: pytest.FixtureRequest) -> Mode:
    return request.param


@pytest.mark.parametrize("dialect", ["postgresql", "mysql", "sqlite"])
@pytest.mark.parametrize("operator", ["like", "nlike", "ilike", "nilike"])
@pytest.mark.parametrize("value", ["50\\", "a\\\\\\"])
async def test_like_pattern_ending_with_escape_is_rejected(
    dialect: SupportedDialect, operator: str, value: str, mode: Mode
) -> None:
    execute = _executor(dialect, mode)
    result = await maybe_async(
        execute(f"{{ users(filter: {{ name: {{ {operator}: {json.dumps(value)} }} }}) {{ name }} }}")
    )
    assert result.errors
    assert len(result.errors) == 1
    assert result.errors[0].message == f"LIKE pattern {value!r} must not end with an escape character"


@pytest.mark.parametrize(
    "relation_input",
    [
        pytest.param("set: [ {{ id: {fruit_id} }} ], add: [ {{ id: {fruit_id} }} ]", id="add"),
        pytest.param('set: [ {{ id: {fruit_id} }} ], create: [ {{ name: "new fruit 1", sweetness: 1 }} ]', id="create"),
    ],
)
async def test_create_with_to_many_set_exclusive_with_add_and_create(relation_input: str, mode: Mode) -> None:
    execute = _executor("postgresql", mode)
    fruits = relation_input.format(fruit_id=_gql_id())
    result = await maybe_async(
        execute(f'mutation {{ createColor(data: {{ name: "new color", fruits: {{ {fruits} }} }}) {{ name }} }}')
    )
    assert not result.data
    assert result.errors
    assert len(result.errors) == 1
    assert (
        result.errors[0].args[0] == "You cannot use `set` with `create`, `upsert` or `add` in a -to-many relation input"
    )


@pytest.mark.parametrize(
    "relation_input",
    [
        pytest.param("set: [ {{ id: {fruit_id} }} ] add: [ {{ id: {fruit_id} }} ]", id="add"),
        pytest.param(
            'set: [ {{ id: {fruit_id} }} ] create: [ {{ name: "new fruit 3 during update", sweetness: 1 }} ]',
            id="create",
        ),
        pytest.param("set: [ {{ id: {fruit_id} }} ] remove: [ {{ id: {fruit_id} }} ]", id="remove"),
        pytest.param(
            "set: [ {{ id: {fruit_id} }} ] "
            'upsert: {{ create: [ {{ name: "new fruit 4 during update", sweetness: 1 }} ] conflictFields: id }}',
            id="upsert",
        ),
    ],
)
async def test_update_with_to_many_set_exclusive_with_add_create_remove(relation_input: str, mode: Mode) -> None:
    execute = _executor("postgresql", mode)
    fruits = relation_input.format(fruit_id=_gql_id())
    result = await maybe_async(
        execute(
            f'mutation {{ updateColor(data: {{ id: {_gql_id()}, name: "updated color name", fruits: {{ {fruits} }} }}) '
            "{ id name fruits { id } } }"
        )
    )
    assert not result.data
    assert result.errors
    assert len(result.errors) == 1
    assert (
        result.errors[0].args[0]
        == "You cannot use `set` with `create`, `upsert`, `add` or `remove` in a -to-many relation input"
    )


async def test_update_with_to_one_set_and_create_fail(mode: Mode) -> None:
    execute = _executor("postgresql", mode)
    result = await maybe_async(
        execute(
            f'mutation {{ updateFruit(data: {{ id: {_gql_id()}, name: "updated fruit name", '
            f'color: {{ set: {{ id: {_gql_id()} }}, create: {{ name: "newly created color during update" }} }} }}) '
            "{ id name color { id } } }"
        )
    )
    assert not result.data
    assert result.errors
    assert len(result.errors) == 1
    assert (
        result.errors[0].args[0] == "You cannot use `set` along with `create` or `upsert` in a -to-one relation input"
    )


def test_required_id_single() -> None:
    query, _ = _schema("postgresql", "sync")
    result = generate_query(query=query)("{ user { name } }")

    assert result.errors
    assert len(result.errors) == 1
    assert isinstance(result.errors[0], GraphQLError)
    assert (
        result.errors[0].message == "Argument 'Query.user(id:)' of type 'UUID!' is required, but it was not provided."
    )

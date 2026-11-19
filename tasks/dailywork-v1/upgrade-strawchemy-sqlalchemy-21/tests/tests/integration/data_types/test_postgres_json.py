from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import Insert, MetaData, insert, null

from tests.integration.models import PostgresJSONChildModel, PostgresJSONModel, postgres_json_metadata
from tests.integration.types import postgres as postgres_types
from tests.integration.utils import graphql_input
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.typing import SupportedDialect
    from tests.integration.fixtures import QueryTracker
    from tests.integration.typing import RawRecordData
    from tests.typing import AnyQueryExecutor

pytestmark = [pytest.mark.integration]

_ROWS: RawRecordData = [
    {"id": 1, "dict_col": {"key1": "value1", "key2": 2, "nested": {"inner": "value"}, "key3": 3, "key4": None}},
    {"id": 2, "dict_col": {"status": "pending", "count": 0, "key3": 3, "key4": None}},
    {"id": 3, "dict_col": {"key3": 3, "key4": None}},
    {"id": 4, "dict_col": null()},
]
_CHILD_ROWS: RawRecordData = [
    {"id": 1, "parent_id": 1, "dict_col": {"a": 1}},
    {"id": 2, "parent_id": 1, "dict_col": {"a": 2}},
    {"id": 3, "parent_id": 1, "dict_col": {"a": 1}},
    {"id": 4, "parent_id": 2, "dict_col": {"b": 1, "c": 2}},
]


@pytest.fixture
def metadata() -> MetaData:
    return postgres_json_metadata


@pytest.fixture
def seed_insert_statements() -> list[Insert]:
    return [insert(PostgresJSONModel).values(_ROWS), insert(PostgresJSONChildModel).values(_CHILD_ROWS)]


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.PostgresJSONAsyncQuery
    pytest.skip(f"PostgreSQL json tests can't be run on this dialect: {dialect}")


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.PostgresJSONSyncQuery
    pytest.skip(f"PostgreSQL json tests can't be run on this dialect: {dialect}")


@pytest.mark.parametrize(
    ("comparison", "expected_ids"),
    [
        pytest.param({"contains": {"key1": "value1"}}, [1], id="contains"),
        pytest.param({"containedIn": {"key1": "value1", "key3": 3, "key4": None, "extra": 1}}, [3], id="containedIn"),
        pytest.param({"hasKey": "key1"}, [1], id="hasKey"),
        pytest.param({"hasKeyAll": ["key1", "key2"]}, [1], id="hasKeyAll"),
        pytest.param({"hasKeyAny": ["key1", "status"]}, [1, 2], id="hasKeyAny"),
        pytest.param({"eq": {"key4": None, "key3": 3}}, [3], id="eq"),
        pytest.param({"neq": {"key4": None, "key3": 3}}, [1, 2], id="neq"),
        pytest.param({"in": [{"key3": 3, "key4": None}, {"count": 0}]}, [3], id="in"),
        pytest.param({"nin": [{"key3": 3, "key4": None}]}, [1, 2], id="nin"),
        pytest.param({"isNull": True}, [4], id="isNull"),
        pytest.param({"isNull": False}, [1, 2, 3], id="isNotNull"),
    ],
)
@pytest.mark.parametrize("negated", [pytest.param(False, id="plain"), pytest.param(True, id="not")])
@pytest.mark.snapshot
async def test_postgres_json_filters(
    comparison: dict[str, Any],
    expected_ids: list[int],
    negated: bool,
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that JSON filters on a PostgreSQL ``json`` column compare it as ``jsonb``."""
    ((name, value),) = comparison.items()
    dto_filter = f"{{ dictCol: {{ {name}: {graphql_input(value)} }} }}"
    if negated:
        dto_filter = f"{{ _not: {dto_filter} }}"
        expected_ids = [row["id"] for row in _ROWS if row["id"] not in expected_ids]
    result = await maybe_async(any_async_query(f"{{ json(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data
    assert sorted(row["id"] for row in result.data["json"]) == expected_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_postgres_json_extract_path(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query('{ json { id dictCol(path: "$.nested.inner") } }'))
    assert not result.errors
    assert result.data
    assert {row["id"]: row["dictCol"] for row in result.data["json"]} == {1: "value", 2: {}, 3: {}, 4: {}}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


def _children(**children_ids: list[int]) -> list[dict[str, Any]]:
    return [{"id": int(key[1:]), "children": [{"id": id_} for id_ in ids]} for key, ids in children_ids.items()]


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        pytest.param(
            "{ json(orderBy: [{ dictCol: ASC }]) { id } }",
            [{"id": 3}, {"id": 2}, {"id": 1}, {"id": 4}],
            id="order-by",
        ),
        pytest.param(
            "{ json(orderBy: [{ dictCol: DESC_NULLS_LAST }]) { id } }",
            [{"id": 1}, {"id": 2}, {"id": 3}, {"id": 4}],
            id="order-by-nulls",
        ),
        pytest.param(
            "{ json(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }]) { id } }",
            [{"id": 3}, {"id": 2}, {"id": 1}, {"id": 4}],
            id="distinct-on-order-by",
        ),
        pytest.param(
            "{ jsonPaginated(orderBy: [{ dictCol: ASC }], limit: 2) { id } }",
            [{"id": 3}, {"id": 2}],
            id="paginated-order-by",
        ),
        pytest.param(
            "{ jsonPaginated(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }], limit: 2) { id } }",
            [{"id": 3}, {"id": 2}],
            id="paginated-distinct-on",
        ),
        pytest.param(
            "{ json(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }]) { id children(orderBy: [{ id: ASC }]) { id } } }",
            _children(i3=[], i2=[4], i1=[1, 2, 3], i4=[]),
            id="distinct-on-to-many",
        ),
        pytest.param(
            "{ json(filter: { childrenAggregate: { count: { arguments: [dictCol], distinct: true, predicate: { gt: 1 } } } })"
            " { id } }",
            [{"id": 1}],
            id="count-distinct-filter",
        ),
        pytest.param(
            "{ json(orderBy: [{ id: ASC }]) { id children(orderBy: [{ dictCol: DESC }, { id: ASC }]) { id } } }",
            _children(i1=[2, 1, 3], i2=[4], i3=[], i4=[]),
            id="nested-order-by",
        ),
        pytest.param(
            "{ json(orderBy: [{ id: ASC }]) { id children(orderBy: [{ dictCol: ASC }, { id: ASC }], limit: 1) { id } } }",
            _children(i1=[1], i2=[4], i3=[], i4=[]),
            id="nested-paginated-order-by",
        ),
        pytest.param(
            "{ json(orderBy: [{ id: ASC }]) { id children(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }, { id: DESC }])"
            " { id } } }",
            _children(i1=[3, 2], i2=[4], i3=[], i4=[]),
            id="nested-distinct-on",
        ),
        pytest.param(
            "{ json(distinctOn: [dictCol], orderBy: [{ id: DESC }]) { id } }",
            [{"id": 4}, {"id": 3}, {"id": 2}, {"id": 1}],
            id="distinct-on-other-order-by",
        ),
        pytest.param(
            "{ jsonPaginated(distinctOn: [dictCol], orderBy: [{ id: DESC }], limit: 2) { id } }",
            [{"id": 4}, {"id": 3}],
            id="paginated-distinct-on-other-order-by",
        ),
        pytest.param(
            "{ json(orderBy: [{ id: ASC }]) { id children(distinctOn: [dictCol], orderBy: [{ id: DESC }]) { id } } }",
            _children(i1=[3, 2], i2=[4], i3=[], i4=[]),
            id="nested-distinct-on-other-order-by",
        ),
    ],
)
@pytest.mark.snapshot
async def test_postgres_json_order_by_and_distinct_on(
    query: str,
    expected: list[dict[str, Any]],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that ordering, DISTINCT ON and distinct counts on a PostgreSQL ``json`` column compare it as ``jsonb``."""
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert next(iter(result.data.values())) == expected

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "query",
    [
        pytest.param("{ json(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }]) { id dictCol } }", id="root"),
        pytest.param(
            "{ jsonPaginated(distinctOn: [dictCol], orderBy: [{ dictCol: ASC }], limit: 4) { id dictCol } }",
            id="paginated",
        ),
        pytest.param(
            "{ json(orderBy: [{ id: ASC }]) { id dictCol children(distinctOn: [dictCol]) { dictCol } } }", id="nested"
        ),
    ],
)
@pytest.mark.snapshot
async def test_postgres_json_distinct_on_selects_stored_values(
    query: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that DISTINCT ON returns the ``json`` values as stored, not their ``jsonb`` cast, whose keys are sorted."""
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    rows = {row["id"]: row for row in next(iter(result.data.values()))}
    assert list(rows[1]["dictCol"]) == ["key1", "key2", "nested", "key3", "key4"]
    if "children" in rows[1]:
        assert [child["dictCol"] for child in rows[1]["children"]] == [{"a": 1}, {"a": 2}]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot

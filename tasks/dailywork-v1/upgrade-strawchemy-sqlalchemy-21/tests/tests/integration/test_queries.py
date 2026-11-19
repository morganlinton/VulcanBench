from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from strawberry.types import get_object_definition

from tests.integration.types.postgres import UserType
from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from tests.integration.fixtures import QueryTracker

pytestmark = [pytest.mark.integration]


async def test_single(any_async_query: AnyQueryExecutor, raw_users: RawRecordData) -> None:
    result = await maybe_async(
        any_async_query(
            """
            query GetUser($id: Int!) {
                user(id: $id) {
                    name
            }
            }
            """,
            {"id": raw_users[0]["id"]},
        )
    )

    assert not result.errors
    assert result.data
    assert result.data["user"] == {"name": raw_users[0]["name"]}


async def test_typename_do_not_fail(any_async_query: AnyQueryExecutor, raw_users: RawRecordData) -> None:
    result = await maybe_async(
        any_async_query(
            """
            query GetUser($id: Int!) {
                user(id: $id) {
                    __typename
            }
            }
            """,
            {"id": raw_users[0]["id"]},
        )
    )

    assert not result.errors
    assert result.data
    assert result.data["user"] == {"__typename": get_object_definition(UserType, strict=True).name}


async def test_many(any_async_query: AnyQueryExecutor, raw_users: RawRecordData) -> None:
    result = await maybe_async(any_async_query("{ users { name } }"))

    assert not result.errors
    assert result.data
    assert result.data["users"] == [{"name": user["name"]} for user in raw_users]


async def test_relation(any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData) -> None:
    result = await maybe_async(any_async_query("{ fruits { color { id } } }"))

    assert not result.errors
    assert result.data
    assert result.data["fruits"] == [{"color": {"id": fruit["color_id"]}} for fruit in raw_fruits]


async def test_list_relation(any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData) -> None:
    result = await maybe_async(any_async_query("{ colors { id fruits { name id } } }"))

    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        assert len(color["fruits"]) == len([f for f in raw_fruits if f["color_id"] == color["id"]])
        for fruit in color["fruits"]:
            expected = next(f for f in raw_fruits if f["id"] == fruit["id"])
            assert fruit == {"name": expected["name"], "id": expected["id"]}


async def test_repeated_root_field_merges_selections(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, raw_colors: RawRecordData
) -> None:
    """Test that a root field selected twice under one response key loads the columns of both selections."""
    result = await maybe_async(any_async_query("{ colors { id } colors { name } }"))

    assert not result.errors
    assert result.data
    assert query_tracker.query_count == 1
    assert sorted(result.data["colors"], key=lambda color: color["id"]) == raw_colors


async def test_column_property(any_async_query: AnyQueryExecutor, raw_users: RawRecordData) -> None:
    result = await maybe_async(
        any_async_query(
            """
            query GetUser($id: Int!) {
                user(id: $id) {
                    greeting
            }
            }
            """,
            {"id": raw_users[0]["id"]},
        )
    )

    assert not result.errors
    assert result.data
    assert result.data["user"] == {"greeting": f"Hello, {raw_users[0]['name']}"}


@pytest.mark.snapshot
async def test_only_queried_columns_included_in_select(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    await maybe_async(any_async_query("{ colors { name fruits { name id } } }"))
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_filtered_statement(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query("{ colorsFiltered { name } }"))

    assert not result.errors
    assert result.data
    assert len(result.data["colorsFiltered"]) == 1
    assert result.data["colorsFiltered"][0]["name"] == "Red"

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("limit", "offset", "expected"),
    [
        pytest.param(1, 0, ["Red"], id="first-page"),
        pytest.param(1, 1, ["Green"], id="second-page"),
        pytest.param(2, 0, ["Red", "Green"], id="full-set"),
    ],
)
async def test_filter_statement_pagination(
    any_async_query: AnyQueryExecutor, limit: int, offset: int, expected: list[str]
) -> None:
    """filter_statement must scope rows BEFORE limit/offset."""
    query = f"{{ colorsFilteredPaginated(limit: {limit}, offset: {offset}, orderBy: [{{id: ASC}}]) {{ name }} }}"
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert [c["name"] for c in result.data["colorsFilteredPaginated"]] == expected


@pytest.mark.parametrize(
    ("offset", "expected"),
    [
        # Visible names sorted ASC: Green, Pink, Red
        pytest.param(0, ["Green"], id="first"),
        pytest.param(1, ["Pink"], id="second"),
        pytest.param(2, ["Red"], id="third"),
    ],
)
async def test_filter_statement_distinct_pagination(
    any_async_query: AnyQueryExecutor, offset: int, expected: list[str]
) -> None:
    """filter_statement must scope rows before DISTINCT ON ranking and pagination."""
    query = (
        f"{{ colorsFilteredDistinct(distinctOn: [name], orderBy: [{{name: ASC}}], limit: 1, offset: {offset})"
        " { name } }"
    )
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert [c["name"] for c in result.data["colorsFilteredDistinct"]] == expected


@pytest.mark.snapshot
async def test_secondary_table_relationships(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_users: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query("{ users { id departments { id name } } }"))

    assert not result.errors
    assert result.data

    assert len(result.data["users"]) == len(raw_users)
    assert result.data["users"] == [
        {"id": 1, "departments": [{"id": 1, "name": "IT"}]},
        {"id": 2, "departments": [{"id": 2, "name": "Sales"}]},
        {"id": 3, "departments": [{"id": 1, "name": "IT"}, {"id": 3, "name": "Platform"}]},
        {"id": 4, "departments": []},
    ]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_filter_statement_pagination_sql(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query("{ colorsFilteredPaginated(limit: 1, offset: 1, orderBy: [{id: ASC}]) { name } }")
    )
    assert not result.errors
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot

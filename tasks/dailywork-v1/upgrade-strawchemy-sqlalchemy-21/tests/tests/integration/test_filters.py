from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

pytestmark = [pytest.mark.integration]

_ALL_USERS = ["Alice", "Bob", "Charlie", "Tango"]
_TEXT_OPERATORS = (
    "eq",
    "neq",
    "isNull",
    "in",
    "nin",
    "gt",
    "gte",
    "lt",
    "lte",
    "like",
    "nlike",
    "ilike",
    "nilike",
    "regexp",
    "iregexp",
    "nregexp",
    "inregexp",
    "startswith",
    "endswith",
    "contains",
    "istartswith",
    "iendswith",
    "icontains",
)


@pytest.mark.snapshot
async def test_no_filtering(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(any_async_query("{ fruits { id } }"))
    assert not result.errors
    assert result.data
    assert len(result.data["fruits"]) == len(raw_fruits)
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_eq(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { name: { eq: "Apple" } }) {
                id
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["fruits"]) == 1
    assert result.data["fruits"][0] == {
        "id": next(fruit["id"] for fruit in raw_fruits if fruit["name"] == "Apple"),
        "name": "Apple",
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_neq(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { name: { neq: "Apple" } }) {
                id
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["fruits"]) == len(raw_fruits) - 1

    assert result.data["fruits"] == [
        {"id": fruit["id"], "name": fruit["name"]} for fruit in raw_fruits if fruit["name"] != "Apple"
    ]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_isnull(
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            users(filter: { bio: { isNull: true } }) {
                id
                bio
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["users"]) == len(raw_users) - 1
    assert result.data["users"] == [{"id": user["id"], "bio": user["bio"]} for user in raw_users if user["bio"] is None]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


# Tests for in and nin filters
@pytest.mark.snapshot
async def test_in(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { in: [ 1, 9 ] } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] in {1, 9}]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_nin(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { nin: [ 1, 9 ] } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] not in {1, 9}]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_gt(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { gt: 10 } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] > 10]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_gte(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { gte: 9 } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] >= 9]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_lt(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { lt: 1 } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] < 1]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_lte(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: { sweetness: { lte: 1 } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] <= 1]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


# Tests for string-specific filters
@pytest.mark.parametrize(
    ("filter_name", "value", "expected_ids"),
    [
        pytest.param("like", "%Appl%", [0], id="like"),
        pytest.param("nlike", "%Apple%", list(range(1, 11)), id="nlike"),
        pytest.param("ilike", "%appl%", [0], id="ilike"),
        pytest.param("nilike", "%appl%", list(range(1, 11)), id="nilike"),
        pytest.param("startswith", "Appl", [0], id="startswith"),
        pytest.param("endswith", "pple", [0], id="endswith"),
        pytest.param("contains", "Water", [9], id="contains"),
        pytest.param("istartswith", "appl", [0], id="istartswith"),
        pytest.param("iendswith", "PPLE", [0], id="iendswith"),
        pytest.param("icontains", "water", [9], id="icontains"),
        pytest.param("regexp", "^c.*", [6], id="regexp"),
        pytest.param("iregexp", "^c.*", [1, 6, 8], id="iregexp"),
        pytest.param("nregexp", "^c.*", [0, 1, 2, 3, 4, 5, 7, 8, 9, 10], id="nregexp"),
        pytest.param("inregexp", "^c.*", [0, 2, 3, 4, 5, 7, 9, 10], id="inregexp"),
    ],
)
@pytest.mark.snapshot
async def test_string_filters(
    filter_name: str,
    value: str,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            fruits(filter: {{ name: {{ {filter_name}: {to_graphql_representation(value, "input")} }} }}) {{
                id
                name
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["fruits"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["fruits"][i]["id"] == raw_fruits[expected_id]["id"]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


# Tests for logical operators
@pytest.mark.snapshot
async def test_and(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: {
                _and: [
                    { sweetness: { gt: 8 } },
                    { name: { contains: "erry" } }
                ]
            }) {
                id
                name
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] > 8 and "erry" in fruit["name"]]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_or(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: {
                _or: [
                    { sweetness: { gt: 8 } },
                    { name: { contains: "erry" } }
                ]
            }) {
                id
                name
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    expected = [fruit for fruit in raw_fruits if fruit["sweetness"] > 8 or "erry" in fruit["name"]]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_not(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:  # sourcery skip: de-morgan
    query = """
        {
            fruits(filter: { _not: { sweetness: { lt: 10 } } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    expected = [fruit for fruit in raw_fruits if not fruit["sweetness"] < 10]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


# Test complex nested logical operators
@pytest.mark.snapshot
async def test_complex_logical_operators(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruits(filter: {
                _or: [
                    {
                        _and: [
                            { sweetness: { gt: 0 } },
                            { name: { contains: "erry" } }
                        ]
                    },
                    {
                        _and: [
                            { waterPercent: { gt: 0.8 } }
                            { sweetness: { lt: 6 } }
                        ]
                    },
                ]
            }) {
                id
                sweetness
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    expected = [
        fruit
        for fruit in raw_fruits
        if (fruit["sweetness"] > 0 and "erry" in fruit["name"])
        or (fruit["water_percent"] > 0.8 and fruit["sweetness"] < 6)
    ]
    assert len(result.data["fruits"]) == len(expected)
    assert {result.data["fruits"][i]["id"] for i in range(len(expected))} == {fruit["id"] for fruit in expected}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_filter_on_paginated_query(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            fruitsPaginatedDefaultLimit1(filter: { _not: { sweetness: { gt: 11 } } }) {
                id
                sweetness
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    expected = [raw_fruits[0]]
    assert len(result.data["fruitsPaginatedDefaultLimit1"]) == len(expected)
    assert {result.data["fruitsPaginatedDefaultLimit1"][i]["id"] for i in range(len(expected))} == {
        fruit["id"] for fruit in expected
    }

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


def _names_by_color(colors: list[dict[str, Any]]) -> dict[str, list[str]]:
    return {color["name"]: sorted(fruit["name"] for fruit in color["fruits"]) for color in colors}


@pytest.mark.snapshot
async def test_to_many_filter_keeps_every_selected_related_row(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a filter on a to-many relation selects parents without restricting the selected relation.

    Regression test for #290: the filter join was reused to load the selection, which only kept the matching rows.
    """
    query = """
        {
            colors(filter: { fruits: { sweetness: { lte: 5 } } }) {
                name
                fruits { name }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert _names_by_color(result.data["colors"]) == {
        "Red": ["Apple", "Cherry"],
        "Yellow": ["Banana", "Lemon", "Quince"],
        "Green": ["Cantaloupe", "Strawberry"],
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_to_many_filter_on_paginated_field_keeps_every_selected_related_row(
    any_async_query: AnyQueryExecutor,
) -> None:
    """Test that a paginated field and a plain one return the same related rows under a to-many filter."""
    query = """
        {
            colorsFilterablePaginated(filter: { fruits: { sweetness: { lte: 5 } } }) {
                name
                fruits { name }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert _names_by_color(result.data["colorsFilterablePaginated"]) == {
        "Red": ["Apple", "Cherry"],
        "Yellow": ["Banana", "Lemon", "Quince"],
        "Green": ["Cantaloupe", "Strawberry"],
    }


@pytest.mark.snapshot
async def test_to_many_filter_paginates_distinct_parents(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a parent matching a to-many filter through several rows takes one slot of the page.

    Regression test for #291: the pagination subquery joined the relation, so LIMIT counted each matching row.
    """
    query = """
        {
            colorsFilterablePaginated(limit: 3, filter: { fruits: { sweetness: { lte: 5 } } }, orderBy: { name: ASC }) {
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["colorsFilterablePaginated"] == [{"name": "Green"}, {"name": "Red"}, {"name": "Yellow"}]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_to_many_filter_in_or_branch_keeps_every_selected_related_row(any_async_query: AnyQueryExecutor) -> None:
    """Test that a to-many filter under ``_or`` selects parents without restricting the selected relation."""
    query = """
        {
            colors(filter: { _or: [{ fruits: { name: { eq: "Apple" } } }, { name: { eq: "Pink" } }] }) {
                name
                fruits { name }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert _names_by_color(result.data["colors"]) == {"Red": ["Apple", "Cherry"], "Pink": ["Pears", "Watermelon"]}


@pytest.mark.snapshot
async def test_to_many_filter_behind_to_one_keeps_every_selected_related_row(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a to-many filter reached through a to-one relation leaves the selected relations whole."""
    query = """
        {
            fruits(filter: { color: { fruits: { name: { eq: "Apple" } } } }) {
                name
                color { name fruits { name } }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert sorted(
        (fruit["name"], fruit["color"]["name"], sorted(sibling["name"] for sibling in fruit["color"]["fruits"]))
        for fruit in result.data["fruits"]
    ) == [("Apple", "Red", ["Apple", "Cherry"]), ("Cherry", "Red", ["Apple", "Cherry"])]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_to_many_filter_and_column_filter(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a column filter next to a to-many filter restricts the parents outside of the EXISTS subquery."""
    query = """
        {
            colors(filter: { name: { neq: "Red" }, fruits: { sweetness: { lte: 5 } } }) {
                name
                fruits { name }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert _names_by_color(result.data["colors"]) == {
        "Yellow": ["Banana", "Lemon", "Quince"],
        "Green": ["Cantaloupe", "Strawberry"],
    }
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_to_many_filter_and_to_one_filter(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a to-one filter next to a to-many filter restricts the parents through a join."""
    query = """
        {
            users(filter: { group: { name: { eq: "Group 1" } }, departments: { name: { eq: "IT" } } }) {
                name
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["users"] == [{"name": "Alice"}]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field", "arguments"),
    [
        pytest.param("users", "", id="plain"),
        pytest.param("usersPaginated", "", id="paginated"),
        pytest.param("users", ", distinctOn: [name]", id="distinct-on"),
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param("{ departments: { name: { isNull: true } } }", [], id="to-many-null"),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { name: { isNull: true } } }', [], id="column-and-to-many-null"
        ),
        pytest.param("{ group: { name: { isNull: true } } }", [], id="to-one-null"),
        pytest.param('{ name: { neq: "Zed" }, group: { name: { isNull: true } } }', [], id="column-and-to-one-null"),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { _or: [{ name: { isNull: true } }, { name: { eq: "IT" } }] } }',
            ["Alice", "Charlie"],
            id="column-and-to-many-or-null",
        ),
        pytest.param(
            '{ group: { name: { isNull: true } }, departments: { name: { eq: "IT" } } }',
            [],
            id="to-one-null-and-to-many",
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { group: { name: { isNull: true } } }] }', ["Bob"], id="or-to-one-null"
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { departments: { name: { isNull: true } } }] }',
            ["Bob"],
            id="or-to-many-null",
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, { group: { name: { eq: "Group 1" } } }] }',
            ["Alice", "Bob"],
            id="or-to-one",
        ),
        pytest.param(
            "{ _not: { group: { name: { isNull: true } } } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-to-one-null",
        ),
        pytest.param(
            '{ name: { neq: "Zed" }, _not: { group: { name: { isNull: true } } } }',
            ["Alice", "Bob", "Charlie", "Tango"],
            id="column-and-not-to-one-null",
        ),
        pytest.param('{ _not: { group: { name: { eq: "Group 1" } } } }', ["Bob", "Charlie", "Tango"], id="not-to-one"),
        pytest.param(
            '{ name: { neq: "Zed" }, _not: { group: { name: { eq: "Group 1" } } } }',
            ["Bob", "Charlie", "Tango"],
            id="column-and-not-to-one",
        ),
        pytest.param("{ departments: { users: { group: { name: { isNull: true } } } } }", [], id="nested-to-one-null"),
        pytest.param(
            '{ name: { neq: "Zed" }, departments: { users: { group: { name: { isNull: true } } } } }',
            [],
            id="column-and-nested-to-one-null",
        ),
        pytest.param(
            '{ departments: { name: { neq: "Zed" }, users: { group: { name: { isNull: true } } } } }',
            [],
            id="nested-column-and-to-one-null",
        ),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } }, "
            "group: { name: { isNull: true } } }",
            [],
            id="aggregation-and-to-one-null",
        ),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } }, "
            "departments: { name: { isNull: true } } }",
            [],
            id="aggregation-and-to-many-null",
        ),
        pytest.param('{ group: { _not: { name: { eq: "Group 1" } } } }', [], id="to-one-not"),
        pytest.param('{ departments: { _not: { name: { eq: "IT" } } } }', ["Bob", "Charlie"], id="to-many-not"),
        pytest.param(
            '{ departments: { _or: [{ name: { eq: "Sales" } }, { _not: { name: { isNull: false } } }] } }',
            ["Bob"],
            id="to-many-or-not",
        ),
        pytest.param(
            "{ departments: { usersAggregate: { count: { arguments: [id], predicate: { lt: 2 } } } } }",
            ["Bob", "Charlie"],
            id="to-many-aggregation",
        ),
        pytest.param(
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }",
            [],
            id="to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ name: { eq: "Bob" } }, '
            "{ group: { topicsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } } }] }",
            ["Bob"],
            id="or-to-one-aggregation",
        ),
        pytest.param(
            '{ _or: [{ group: { name: { eq: "Group 1" } } }, { departments: { name: { eq: "Sales" } } }] }',
            ["Alice", "Bob"],
            id="or-to-one-and-to-many",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            "{ group: { name: { isNull: true } } }] }",
            ["Tango"],
            id="or-aggregation-and-to-one-null",
        ),
        pytest.param(
            "{ _or: [{ departmentsAggregate: { count: { arguments: [id], predicate: { eq: 0 } } } }, "
            '{ group: { name: { eq: "Group 1" } } }] }',
            ["Alice", "Tango"],
            id="or-aggregation-and-to-one",
        ),
    ],
)
async def test_relation_filter_ignores_siblings(
    field: str, arguments: str, dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    """Test that a relation filter only matches rows having a matching related row, whatever its sibling filters."""
    result = await maybe_async(any_async_query(f"{{ {field}(filter: {dto_filter}{arguments}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data[field]) == names


@pytest.mark.snapshot
async def test_not_isnull(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query("{ users(filter: { _not: { bio: { isNull: true } } }) { name } }"))
    assert not result.errors
    assert result.data

    assert result.data["users"] == [{"name": "Tango"}]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param("{ _not: { bio: { isNull: false } } }", ["Alice", "Bob", "Charlie"], id="not-isnull-false"),
        pytest.param('{ _not: { bio: { eq: "Tango\'s bio" } } }', ["Alice", "Bob", "Charlie"], id="not-eq"),
        pytest.param(
            '{ _not: { bio: { isNull: true, eq: "Tango\'s bio" } } }',
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-isnull-and-eq",
        ),
        pytest.param(
            '{ _not: { _or: [{ bio: { isNull: true } }, { name: { eq: "Bob" } }] } }', ["Tango"], id="not-or-isnull"
        ),
        pytest.param("{ _not: { _not: { bio: { isNull: true } } } }", ["Alice", "Bob", "Charlie"], id="not-not-isnull"),
    ],
)
async def test_not_isnull_column(dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor) -> None:
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    ("raw_users", "raw_user_departments"),
    [
        pytest.param(
            [
                {"id": 1, "name": "Alice", "group_id": 1, "bio": None},
                {"id": 2, "name": "Bob", "group_id": 1, "bio": None},
                {"id": 3, "name": "Charlie", "group_id": 2, "bio": None},
                {"id": 4, "name": "Dave", "group_id": 1, "bio": None},
                {"id": 5, "name": "Eve", "group_id": None, "bio": None},
            ],
            [
                {"user_id": 1, "department_id": 1},
                {"user_id": 2, "department_id": 2},
                {"user_id": 3, "department_id": 1},
                {"user_id": 4, "department_id": 1},
                {"user_id": 4, "department_id": 3},
                {"user_id": 5, "department_id": 3},
            ],
            id="it-three-users-platform-two",
        )
    ],
)
async def test_aggregate_under_one_of_two_to_many_paths(
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
    raw_user_departments: RawRecordData,  # noqa: ARG001
) -> None:
    """A user passes a count over its departments' users and a topic of its group when both hold for some rows."""
    dto_filter = (
        "{ departments: { usersAggregate: { count: { predicate: { gt: 1 } } } }, "
        'group: { topics: { name: { eq: "Hello!" } } } }'
    )

    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))

    assert not result.errors
    assert result.data is not None
    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Dave"]


@pytest.mark.parametrize(
    ("raw_groups", "raw_users", "raw_departments"),
    [
        pytest.param(
            [
                {"id": 1, "name": "Group 1"},
                {"id": 2, "name": None},
                *({"id": i, "name": f"Group {i}"} for i in (3, 4, 5)),
            ],
            [
                {"id": 1, "name": "Alice", "group_id": 1, "bio": None},
                {"id": 2, "name": "Bob", "group_id": 2, "bio": None},
                {"id": 3, "name": "Charlie", "group_id": None, "bio": None},
                {"id": 4, "name": "Tango", "group_id": None, "bio": "Tango's bio"},
            ],
            [{"id": 1, "name": "IT"}, {"id": 2, "name": None}, {"id": 3, "name": "Platform"}],
            id="null-names",
        )
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param("{ group: { name: { isNull: true } } }", ["Bob"], id="to-one-isnull"),
        pytest.param(
            "{ _not: { group: { name: { isNull: true } } } }", ["Alice", "Charlie", "Tango"], id="not-to-one-isnull"
        ),
        pytest.param(
            "{ _not: { group: { name: { isNull: false } } } }",
            ["Bob", "Charlie", "Tango"],
            id="not-to-one-isnull-false",
        ),
        pytest.param("{ group: { _not: { name: { isNull: true } } } }", ["Alice"], id="to-one-not-isnull"),
        pytest.param("{ departments: { name: { isNull: true } } }", ["Bob"], id="to-many-isnull"),
        pytest.param(
            "{ _not: { departments: { name: { isNull: true } } } }",
            ["Alice", "Charlie", "Tango"],
            id="not-to-many-isnull",
        ),
        pytest.param(
            "{ departments: { _not: { name: { isNull: true } } } }", ["Alice", "Charlie"], id="to-many-not-isnull"
        ),
    ],
)
async def test_not_isnull_relation(
    dto_filter: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_groups: RawRecordData,  # noqa: ARG001
    raw_users: RawRecordData,  # noqa: ARG001
    raw_departments: RawRecordData,  # noqa: ARG001
) -> None:
    """Test ``isNull`` on the columns of NULL-named related rows, directly and under ``_not``."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.snapshot
async def test_not_empty_comparison(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query("{ users(filter: { _not: { bio: {} } }) { name } }"))
    assert not result.errors
    assert result.data

    assert [user["name"] for user in result.data["users"]] == ["Alice", "Bob", "Charlie", "Tango"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("query", "variables", "names"),
    [
        pytest.param(
            "query ($bio: String) { users(filter: { _not: { bio: { eq: $bio } } }) { name } }",
            {},
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-omitted-variable",
        ),
        pytest.param(
            '{ users(filter: { _not: { bio: {}, name: { eq: "Bob" } } }) { name } }',
            None,
            ["Alice", "Charlie", "Tango"],
            id="not-empty-and-eq",
        ),
        pytest.param(
            "{ users(filter: { _not: { _and: [{ bio: {} }] } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-and-empty",
        ),
        pytest.param(
            "{ users(filter: { _not: { _or: [{ bio: {} }] } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-or-empty",
        ),
        pytest.param(
            '{ users(filter: { _not: { _or: [{ bio: {} }, { name: { eq: "Bob" } }] } }) { name } }',
            None,
            ["Alice", "Charlie", "Tango"],
            id="not-or-empty-and-eq",
        ),
        pytest.param(
            "{ users(filter: { _not: { _not: { bio: {} } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-not-empty",
        ),
        pytest.param(
            "{ users(filter: { group: { _not: { name: {} } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="to-one-not-empty",
        ),
        pytest.param(
            "{ users(filter: { departments: { _not: { name: {} } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="to-many-not-empty",
        ),
        pytest.param(
            "{ users(filter: { _not: { departmentsAggregate: { count: { arguments: [id], predicate: {} } } } }) "
            "{ name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-empty-aggregation",
        ),
    ],
)
async def test_empty_comparison_is_ignored(
    query: str, variables: dict[str, Any] | None, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    result = await maybe_async(any_async_query(query, variables))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    ("query", "variables"),
    [
        pytest.param("{ users(filter: { group: {} }) { name } }", None, id="to-one"),
        pytest.param("{ users(filter: { group: { name: {} } }) { name } }", None, id="to-one-empty-comparison"),
        pytest.param(
            "query ($name: String) { users(filter: { group: { name: { eq: $name } } }) { name } }",
            {},
            id="to-one-omitted-variable",
        ),
        pytest.param("{ users(filter: { _not: { group: {} } }) { name } }", None, id="not-to-one"),
        pytest.param(
            "{ users(filter: { _not: { group: { name: {} } } }) { name } }", None, id="not-to-one-empty-comparison"
        ),
        pytest.param("{ users(filter: { group: { topics: {} } }) { name } }", None, id="to-one-to-many"),
        pytest.param(
            "{ users(filter: { group: { topics: { name: {} } } }) { name } }",
            None,
            id="to-one-to-many-empty-comparison",
        ),
        pytest.param("{ users(filter: { group: { _and: [{ name: {} }] } }) { name } }", None, id="to-one-and"),
        pytest.param("{ users(filter: { group: { _or: [{ name: {} }] } }) { name } }", None, id="to-one-or"),
        pytest.param("{ users(filter: { departments: {} }) { name } }", None, id="to-many"),
        pytest.param("{ users(filter: { departments: { name: {} } }) { name } }", None, id="to-many-empty-comparison"),
        pytest.param("{ users(filter: { _not: { departments: {} } }) { name } }", None, id="not-to-many"),
        pytest.param(
            "{ users(filter: { _not: { departments: { name: {} } } }) { name } }",
            None,
            id="not-to-many-empty-comparison",
        ),
        pytest.param(
            "{ users(filter: { departments: { users: { name: {} } } }) { name } }", None, id="to-many-to-many"
        ),
        pytest.param(
            "{ users(filter: { _and: [{ departments: { name: {} } }, { group: {} }] }) { name } }",
            None,
            id="and-relations",
        ),
        pytest.param("{ users(filter: { departmentsAggregate: {} }) { name } }", None, id="aggregation"),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: {} } } }) { name } }",
            None,
            id="aggregation-empty-predicate",
        ),
        pytest.param("{ users(filter: { _not: { departmentsAggregate: {} } }) { name } }", None, id="not-aggregation"),
        pytest.param(
            "{ users(filter: { departments: { usersAggregate: {} } }) { name } }", None, id="to-many-aggregation"
        ),
    ],
)
async def test_empty_relation_filter_is_ignored(
    query: str, variables: dict[str, Any] | None, any_async_query: AnyQueryExecutor
) -> None:
    result = await maybe_async(any_async_query(query, variables))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Bob", "Charlie", "Tango"]


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ group: { name: {} }, name: { eq: "Bob" } }', ["Bob"], id="to-one-and-column"),
        pytest.param('{ departments: { name: {} }, name: { eq: "Bob" } }', ["Bob"], id="to-many-and-column"),
        pytest.param(
            '{ departments: { name: {} }, group: { name: { eq: "Group 1" } } }', ["Alice"], id="to-many-and-to-one"
        ),
        pytest.param('{ _not: { group: { name: {} }, name: { eq: "Bob" } } }', ["Alice", "Charlie", "Tango"], id="not"),
    ],
)
async def test_empty_relation_filter_beside_predicate(
    dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.snapshot
@pytest.mark.parametrize(
    "dto_filter",
    [
        pytest.param("{ group: { name: {} } }", id="to-one"),
        pytest.param("{ departments: { name: {} } }", id="to-many"),
        pytest.param("{ _not: { departments: { name: {} } } }", id="not-to-many"),
    ],
)
async def test_empty_relation_filter(
    dto_filter: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data

    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Bob", "Charlie", "Tango"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param("{ _or: [] }", ["Alice", "Bob", "Charlie", "Tango"], id="no-branch"),
        pytest.param("{ _or: [{}, {}] }", ["Alice", "Bob", "Charlie", "Tango"], id="empty-branches"),
        pytest.param("{ _or: [{ bio: {} }] }", ["Alice", "Bob", "Charlie", "Tango"], id="empty-comparison"),
        pytest.param('{ _or: [{}, { name: { eq: "Bob" } }] }', ["Bob"], id="empty-and-eq"),
        pytest.param('{ _or: [{ bio: {} }, { name: { eq: "Bob" } }] }', ["Bob"], id="empty-comparison-and-eq"),
        pytest.param('{ _or: [{ _not: { bio: {} } }, { name: { eq: "Bob" } }] }', ["Bob"], id="empty-not-and-eq"),
        pytest.param('{ _or: [{ _and: [{ bio: {} }] }, { name: { eq: "Bob" } }] }', ["Bob"], id="empty-and-and-eq"),
        pytest.param('{ _or: [{ group: {} }, { name: { eq: "Bob" } }] }', ["Bob"], id="to-one-and-eq"),
        pytest.param('{ _or: [{ group: { name: {} } }, { name: { eq: "Bob" } }] }', ["Bob"], id="to-one-empty-and-eq"),
        pytest.param('{ _or: [{ departments: {} }, { name: { eq: "Bob" } }] }', ["Bob"], id="to-many-and-eq"),
        pytest.param(
            '{ _or: [{ departments: { name: {} } }, { name: { eq: "Bob" } }] }', ["Bob"], id="to-many-empty-and-eq"
        ),
        pytest.param(
            '{ _or: [{ departmentsAggregate: {} }, { name: { eq: "Bob" } }] }', ["Bob"], id="aggregation-and-eq"
        ),
        pytest.param(
            '{ group: { _or: [{ name: {} }, { name: { eq: "Group 1" } }] } }', ["Alice"], id="to-one-or-empty-and-eq"
        ),
        pytest.param(
            '{ departments: { _or: [{ name: {} }, { name: { eq: "IT" } }] } }',
            ["Alice", "Charlie"],
            id="to-many-or-empty-and-eq",
        ),
        pytest.param(
            '{ _not: { _or: [{ group: { name: {} } }, { name: { eq: "Bob" } }] } }',
            ["Alice", "Charlie", "Tango"],
            id="not-or-to-one-empty-and-eq",
        ),
        pytest.param(
            '{ _not: { _or: [{ departments: { name: {} } }, { name: { eq: "Bob" } }] } }',
            ["Alice", "Charlie", "Tango"],
            id="not-or-to-many-empty-and-eq",
        ),
        pytest.param(
            '{ _or: [{ _or: [{ bio: {} }] }, { name: { eq: "Bob" } }] }', ["Bob"], id="nested-or-empty-and-eq"
        ),
    ],
)
async def test_empty_or_branch_is_pruned(dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor) -> None:
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param("{ name: null }", ["Alice", "Bob", "Charlie", "Tango"], id="column"),
        pytest.param("{ group: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one"),
        pytest.param("{ group: { name: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-column"),
        pytest.param("{ group: { topics: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-to-many"),
        pytest.param("{ departments: null }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many"),
        pytest.param("{ departments: { name: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-column"),
        pytest.param("{ departmentsAggregate: null }", ["Alice", "Bob", "Charlie", "Tango"], id="aggregation"),
        pytest.param(
            "{ departmentsAggregate: { count: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="aggregation-function"
        ),
        pytest.param(
            "{ departments: { usersAggregate: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-many-aggregation"
        ),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: null, predicate: { gt: 0 } } } }",
            ["Alice", "Bob", "Charlie"],
            id="aggregation-arguments",
        ),
        pytest.param(
            "{ _not: { departmentsAggregate: { count: { arguments: null, predicate: { gt: 0 } } } } }",
            ["Tango"],
            id="not-aggregation-arguments",
        ),
        pytest.param("{ _not: null }", ["Alice", "Bob", "Charlie", "Tango"], id="not"),
        pytest.param("{ _not: { name: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-column"),
        pytest.param("{ _not: { group: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-to-one"),
        pytest.param("{ _not: { departments: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="not-to-many"),
        pytest.param(
            "{ _not: { departmentsAggregate: { count: null } } }",
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-aggregation-function",
        ),
        pytest.param("{ group: { _not: null } }", ["Alice", "Bob", "Charlie", "Tango"], id="to-one-not"),
        pytest.param("{ _and: [{ group: null }] }", ["Alice", "Bob", "Charlie", "Tango"], id="and"),
        pytest.param("{ _or: [{ departments: null }] }", ["Alice", "Bob", "Charlie", "Tango"], id="or"),
        pytest.param('{ group: null, name: { eq: "Bob" } }', ["Bob"], id="to-one-and-column"),
        pytest.param('{ departmentsAggregate: null, name: { eq: "Bob" } }', ["Bob"], id="aggregation-and-column"),
        pytest.param('{ _not: null, name: { eq: "Bob" } }', ["Bob"], id="not-and-column"),
        pytest.param('{ _or: [{ departments: null }, { name: { eq: "Bob" } }] }', ["Bob"], id="or-to-many-and-column"),
        pytest.param(
            '{ departments: null, _and: [{ departments: { name: { eq: "IT" } } }] }',
            ["Alice", "Charlie"],
            id="to-many-and-to-many-branch",
        ),
        pytest.param(
            '{ _not: { departments: { name: null }, name: { eq: "Bob" } } }',
            ["Alice", "Charlie", "Tango"],
            id="not-to-many-and-column",
        ),
    ],
)
async def test_null_filter_field_is_ignored(
    dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    """Test that a filter field set to null filters nothing, as if it were absent."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    "query",
    [
        pytest.param("query ($value: GroupBoolExp) { users(filter: { group: $value }) { name } }", id="to-one"),
        pytest.param("query ($value: TextComparison) { users(filter: { name: $value }) { name } }", id="column"),
        pytest.param("query ($value: UserFilter) { users(filter: $value) { name } }", id="filter"),
    ],
)
async def test_null_filter_variable_is_ignored(query: str, any_async_query: AnyQueryExecutor) -> None:
    """Test that a filter bound to a null variable filters nothing."""
    result = await maybe_async(any_async_query(query, {"value": None}))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == ["Alice", "Bob", "Charlie", "Tango"]


@pytest.mark.parametrize(
    ("raw_departments", "raw_user_departments"),
    [
        pytest.param(
            [
                {"id": 1, "name": "IT"},
                {"id": 2, "name": "Sales"},
                {"id": 3, "name": "Platform"},
                {"id": 4, "name": None},
            ],
            [
                {"user_id": 1, "department_id": 1},
                {"user_id": 1, "department_id": 4},
                {"user_id": 2, "department_id": 2},
                {"user_id": 3, "department_id": 3},
                {"user_id": 3, "department_id": 1},
            ],
            id="null-department",
        )
    ],
)
@pytest.mark.parametrize(
    "raw_topics",
    [
        pytest.param(
            [
                {"id": 1, "name": "Hello!", "group_id": 1},
                {"id": 2, "name": "Problems", "group_id": 2},
                {"id": 3, "name": "Welcome", "group_id": 1},
            ],
            id="group-with-two-topics",
        )
    ],
)
@pytest.mark.parametrize(
    ("field", "arguments"),
    [
        pytest.param("users", "", id="plain"),
        pytest.param("usersPaginated", "", id="paginated"),
        pytest.param("users", ", distinctOn: [name]", id="distinct-on"),
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ departments: { name: { eq: "IT" } } }', ["Bob", "Tango"], id="to-many"),
        pytest.param('{ departments: { users: { name: { eq: "Alice" } } } }', ["Bob", "Tango"], id="nested-to-many"),
        pytest.param(
            '{ departments: { _not: { users: { name: { eq: "Alice" } } } } }',
            ["Alice", "Tango"],
            id="to-many-not-nested-to-many",
        ),
        pytest.param(
            '{ _and: [{ departments: { name: { eq: "IT" } } }, { name: { neq: "Alice" } }] }',
            ["Alice", "Bob", "Tango"],
            id="and-to-many",
        ),
        pytest.param(
            '{ _or: [{ departments: { name: { eq: "Sales" } } }, { name: { eq: "Alice" } }] }',
            ["Charlie", "Tango"],
            id="or-to-many-and-column",
        ),
        pytest.param(
            '{ name: { eq: "Charlie" }, departments: { name: { eq: "IT" } } }',
            ["Alice", "Bob", "Tango"],
            id="column-and-to-many",
        ),
        pytest.param(
            '{ group: { name: { eq: "Group 1" } }, departments: { name: { eq: "IT" } } }',
            ["Bob", "Charlie", "Tango"],
            id="to-one-and-to-many",
        ),
        pytest.param(
            '{ _or: [{ group: { name: { eq: "Group 1" } } }, { departments: { name: { eq: "Sales" } } }] }',
            ["Charlie", "Tango"],
            id="or-to-one-and-to-many",
        ),
        pytest.param('{ _not: { departments: { name: { eq: "IT" } } } }', ["Alice", "Charlie"], id="not-to-many"),
        pytest.param('{ departments: { _not: { name: { eq: "IT" } } } }', ["Tango"], id="to-many-not"),
        pytest.param(
            "{ departmentsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } } }",
            ["Bob", "Tango"],
            id="aggregation",
        ),
        pytest.param('{ group: { name: { eq: "Group 1" } } }', ["Bob", "Charlie", "Tango"], id="to-one"),
        pytest.param(
            '{ group: { topics: { name: { eq: "Hello!" } } } }', ["Bob", "Charlie", "Tango"], id="to-one-to-many"
        ),
        pytest.param(
            '{ group: { _not: { topics: { name: { eq: "Problems" } } } } }',
            ["Bob", "Charlie", "Tango"],
            id="to-one-not-to-many",
        ),
        pytest.param(
            '{ _and: [{ departments: { name: { eq: "IT" } } }, { departments: { name: { eq: "Platform" } } }] }',
            ["Alice", "Bob", "Tango"],
            id="and-same-to-many",
        ),
        pytest.param(
            '{ departments: { name: { eq: "IT" } }, _and: [{ departments: { name: { eq: "Platform" } } }] }',
            ["Alice", "Bob", "Tango"],
            id="to-many-and-same-to-many",
        ),
        pytest.param(
            '{ _and: [{ _and: [{ departments: { name: { eq: "IT" } } }] }, '
            '{ departments: { name: { eq: "Platform" } } }] }',
            ["Alice", "Bob", "Tango"],
            id="nested-and-same-to-many",
        ),
        pytest.param(
            '{ departments: { name: { eq: "IT" } }, '
            '_or: [{ departments: { name: { eq: "Platform" } } }, { name: { eq: "Bob" } }] }',
            ["Alice", "Bob", "Tango"],
            id="to-many-and-or-same-to-many",
        ),
        pytest.param(
            '{ _and: [{ _or: [{ departments: { name: { eq: "Sales" } } }, '
            '{ departments: { name: { eq: "Platform" } } }] }, { departments: { name: { eq: "IT" } } }] }',
            ["Alice", "Bob", "Tango"],
            id="and-or-same-to-many",
        ),
        pytest.param(
            '{ _or: [{ departments: { name: { eq: "IT" } } }, { departments: { name: { eq: "Platform" } } }] }',
            ["Bob", "Tango"],
            id="or-same-to-many",
        ),
        pytest.param(
            '{ departments: { _and: [{ name: { eq: "IT" } }, { name: { eq: "Platform" } }] } }',
            ["Alice", "Bob", "Charlie", "Tango"],
            id="to-many-and-same-row",
        ),
        pytest.param(
            '{ _and: [{ _not: { departments: { name: { eq: "IT" } } } }, { departments: { name: { eq: "Sales" } } }] }',
            ["Alice", "Charlie", "Tango"],
            id="and-not-to-many",
        ),
        pytest.param(
            '{ _and: [{ departments: { name: { eq: "Platform" } } }, '
            '{ departments: { users: { name: { eq: "Alice" } } } }] }',
            ["Alice", "Bob", "Tango"],
            id="and-same-to-many-nested-to-many",
        ),
        pytest.param(
            '{ departments: { _and: [{ users: { name: { eq: "Alice" } } }, { users: { name: { eq: "Charlie" } } }] } }',
            ["Bob", "Tango"],
            id="to-many-and-same-nested-to-many",
        ),
        pytest.param(
            '{ group: { _and: [{ topics: { name: { eq: "Hello!" } } }, { topics: { name: { eq: "Welcome" } } }] } }',
            ["Bob", "Charlie", "Tango"],
            id="to-one-and-same-to-many",
        ),
        pytest.param(
            '{ _and: [{ group: { topics: { name: { eq: "Hello!" } } } }, '
            '{ group: { topics: { name: { eq: "Welcome" } } } }] }',
            ["Bob", "Charlie", "Tango"],
            id="and-to-one-same-to-many",
        ),
        pytest.param(
            "{ _and: [{ departmentsAggregate: { count: { arguments: [id], predicate: { gt: 1 } } } }, "
            '{ departments: { name: { eq: "IT" } } }, { departments: { name: { isNull: true } } }] }',
            ["Bob", "Charlie", "Tango"],
            id="and-aggregation-same-to-many",
        ),
    ],
)
async def test_not_complements_relation_filter(
    field: str,
    arguments: str,
    dto_filter: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_departments: RawRecordData,  # noqa: ARG001
    raw_user_departments: RawRecordData,  # noqa: ARG001
    raw_topics: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that ``_not`` over a filter matches exactly the rows the filter does not."""
    query = "{{ {field}(filter: {dto_filter}{arguments}) {{ name }} }}"
    result = await maybe_async(any_async_query(query.format(field=field, dto_filter=dto_filter, arguments=arguments)))
    assert not result.errors
    assert result.data is not None
    matched = [user["name"] for user in result.data[field]]

    negated_filter = f"{{ _not: {dto_filter} }}"
    result = await maybe_async(
        any_async_query(query.format(field=field, dto_filter=negated_filter, arguments=arguments))
    )
    assert not result.errors
    assert result.data is not None
    negated = sorted(user["name"] for user in result.data[field])

    assert negated == names
    assert sorted(matched + negated) == ["Alice", "Bob", "Charlie", "Tango"]


@pytest.mark.snapshot
async def test_not_to_many(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query('{ users(filter: { _not: { departments: { name: { eq: "IT" } } } }) { name } }')
    )
    assert not result.errors
    assert result.data

    assert sorted(user["name"] for user in result.data["users"]) == ["Bob", "Tango"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_and_same_to_many(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(
        any_async_query(
            "{ users(filter: { _and: ["
            '{ departments: { name: { eq: "IT" } } }, { departments: { name: { eq: "Platform" } } }'
            "] }) { name } }"
        )
    )
    assert not result.errors
    assert result.data

    assert [user["name"] for user in result.data["users"]] == ["Charlie"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
@pytest.mark.parametrize("comparison", ["in", "nin"])
async def test_empty_list_comparison(
    comparison: str, any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_async_query(f"{{ users(filter: {{ bio: {{ {comparison}: [] }} }}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert [user["name"] for user in result.data["users"]] == (
        [] if comparison == "in" else ["Alice", "Bob", "Charlie", "Tango"]
    )
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("query", "variables", "names"),
    [
        pytest.param("{ users(filter: { name: { in: [] } }) { name } }", None, [], id="in"),
        pytest.param(
            "{ users(filter: { name: { nin: [] } }) { name } }", None, ["Alice", "Bob", "Charlie", "Tango"], id="nin"
        ),
        pytest.param(
            "{ users(filter: { _not: { bio: { in: [] } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-in",
        ),
        pytest.param("{ users(filter: { _not: { bio: { nin: [] } } }) { name } }", None, [], id="not-nin"),
        pytest.param("{ users(filter: { _not: { _not: { bio: { in: [] } } } }) { name } }", None, [], id="not-not-in"),
        pytest.param(
            '{ users(filter: { _not: { bio: { in: [], eq: "Tango\'s bio" } } }) { name } }',
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-in-and-eq",
        ),
        pytest.param(
            '{ users(filter: { _not: { bio: { nin: [], eq: "Tango\'s bio" } } }) { name } }',
            None,
            ["Alice", "Bob", "Charlie"],
            id="not-nin-and-eq",
        ),
        pytest.param(
            "{ users(filter: { _not: { bio: { nin: [], isNull: true } } }) { name } }",
            None,
            ["Tango"],
            id="not-nin-and-is-null",
        ),
        pytest.param(
            '{ users(filter: { _or: [{ bio: { in: [] } }, { name: { eq: "Bob" } }] }) { name } }',
            None,
            ["Bob"],
            id="or-in",
        ),
        pytest.param(
            "query ($names: [String!]) { users(filter: { name: { in: $names } }) { name } }",
            {"names": []},
            [],
            id="in-variable",
        ),
        pytest.param(
            "{ users(filter: { bio: { in: null } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="in-null",
        ),
        pytest.param(
            "{ users(filter: { _not: { bio: { nin: null } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-nin-null",
        ),
        pytest.param("{ users(filter: { group: { name: { in: [] } } }) { name } }", None, [], id="to-one-in"),
        pytest.param("{ users(filter: { group: { name: { nin: [] } } }) { name } }", None, ["Alice"], id="to-one-nin"),
        pytest.param(
            "{ users(filter: { departments: { name: { nin: [] } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie"],
            id="to-many-nin",
        ),
        pytest.param(
            "{ users(filter: { _not: { departments: { name: { in: [] } } } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-to-many-in",
        ),
        pytest.param(
            "{ users(filter: { _not: { departments: { name: { nin: [] } } } }) { name } }",
            None,
            ["Tango"],
            id="not-to-many-nin",
        ),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: { in: [] } } } }) "
            "{ name } }",
            None,
            [],
            id="aggregation-in",
        ),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: { nin: [] } } } }) "
            "{ name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="aggregation-nin",
        ),
        pytest.param(
            "{ users(filter: { _not: { departmentsAggregate: { count: { arguments: [id], predicate: { in: [] } } } } "
            "}) { name } }",
            None,
            ["Alice", "Bob", "Charlie", "Tango"],
            id="not-aggregation-in",
        ),
    ],
)
async def test_empty_list_comparison_result(
    query: str, variables: dict[str, Any] | None, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    result = await maybe_async(any_async_query(query, variables))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize("operator", _TEXT_OPERATORS)
@pytest.mark.parametrize(
    "template",
    [
        pytest.param("{{ bio: {{ {operator}: null }} }}", id="column"),
        pytest.param("{{ _not: {{ bio: {{ {operator}: null }} }} }}", id="not-column"),
    ],
)
async def test_null_operator_is_ignored(template: str, operator: str, any_async_query: AnyQueryExecutor) -> None:
    """Test that an operator set to null is ignored like an absent one, directly and under ``_not``."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {template.format(operator=operator)}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == _ALL_USERS


@pytest.mark.parametrize(
    ("query", "variables", "names"),
    [
        pytest.param(
            "query ($bio: String) { users(filter: { bio: { eq: $bio } }) { name } }",
            {"bio": None},
            _ALL_USERS,
            id="null-variable",
        ),
        pytest.param(
            "query ($bio: String) { users(filter: { _not: { bio: { neq: $bio } } }) { name } }",
            {"bio": None},
            _ALL_USERS,
            id="not-null-variable",
        ),
        pytest.param(
            "{ users(filter: { bio: { eq: null, isNull: true } }) { name } }",
            None,
            ["Alice", "Bob", "Charlie"],
            id="null-beside-operator",
        ),
        pytest.param(
            '{ users(filter: { _not: { name: { eq: "Bob", neq: null, isNull: null } } }) { name } }',
            None,
            ["Alice", "Charlie", "Tango"],
            id="not-null-beside-operator",
        ),
        pytest.param('{ users(filter: { bio: { eq: null }, name: { eq: "Bob" } }) { name } }', None, ["Bob"], id="and"),
        pytest.param(
            '{ users(filter: { _or: [{ bio: { eq: null } }, { name: { eq: "Bob" } }] }) { name } }',
            None,
            ["Bob"],
            id="or-branch",
        ),
        pytest.param("{ users(filter: { group: { name: { eq: null } } }) { name } }", None, _ALL_USERS, id="to-one"),
        pytest.param(
            "{ users(filter: { group: { name: { isNull: null } } }) { name } }", None, _ALL_USERS, id="to-one-isnull"
        ),
        pytest.param(
            "{ users(filter: { _not: { group: { name: { eq: null } } } }) { name } }",
            None,
            _ALL_USERS,
            id="not-to-one",
        ),
        pytest.param(
            "{ users(filter: { departments: { name: { gt: null } } }) { name } }", None, _ALL_USERS, id="to-many"
        ),
        pytest.param(
            "{ users(filter: { _not: { departments: { name: { like: null } } } }) { name } }",
            None,
            _ALL_USERS,
            id="not-to-many",
        ),
        pytest.param(
            "{ users(filter: { departments: { _not: { name: { neq: null } } } }) { name } }",
            None,
            _ALL_USERS,
            id="to-many-not",
        ),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: { eq: null } } } }) "
            "{ name } }",
            None,
            _ALL_USERS,
            id="aggregation-eq",
        ),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: { gt: null, "
            "isNull: null, in: null } } } }) { name } }",
            None,
            _ALL_USERS,
            id="aggregation-operators",
        ),
        pytest.param(
            "{ users(filter: { _not: { departmentsAggregate: { count: { arguments: [id], "
            "predicate: { lte: null } } } } }) { name } }",
            None,
            _ALL_USERS,
            id="not-aggregation",
        ),
        pytest.param(
            "{ users(filter: { departmentsAggregate: { count: { arguments: [id], predicate: { eq: 1, neq: null } } } }) "
            "{ name } }",
            None,
            ["Alice", "Bob"],
            id="aggregation-null-beside-operator",
        ),
    ],
)
async def test_null_operator_is_ignored_in_filter(
    query: str, variables: dict[str, Any] | None, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    """Test that null operators are ignored beside other operators, in branches, relations and aggregations."""
    result = await maybe_async(any_async_query(query, variables))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ name: { regexp: "o", nregexp: "^B" } }', ["Tango"], id="regexp-and-nregexp"),
        pytest.param('{ name: { iregexp: "a", inregexp: "^t" } }', ["Alice", "Charlie"], id="iregexp-and-inregexp"),
        pytest.param('{ name: { regexp: "" } }', _ALL_USERS, id="regexp-empty"),
        pytest.param('{ name: { nregexp: "" } }', [], id="nregexp-empty"),
        pytest.param('{ name: { iregexp: "" } }', _ALL_USERS, id="iregexp-empty"),
        pytest.param('{ name: { inregexp: "" } }', [], id="inregexp-empty"),
        pytest.param('{ name: { regexp: null, nregexp: "^B" } }', ["Alice", "Charlie", "Tango"], id="regexp-null"),
        pytest.param('{ name: { regexp: "^B", nregexp: null } }', ["Bob"], id="nregexp-null"),
        pytest.param(
            '{ _not: { name: { regexp: "o", nregexp: "^B" } } }',
            ["Alice", "Bob", "Charlie"],
            id="not-regexp-and-nregexp",
        ),
    ],
)
async def test_regexp_operators_are_independent(
    dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    """Test that each regexp operator gets its own predicate, an empty pattern matching every string."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ name: { iregexp: "^A" } }', ["Alice"], id="iregexp-uppercase"),
        pytest.param('{ name: { iregexp: "^a" } }', ["Alice"], id="iregexp-lowercase"),
        pytest.param('{ name: { iregexp: "lI" } }', ["Alice", "Charlie"], id="iregexp-mixed-case"),
        pytest.param('{ name: { iregexp: "^[A-B]" } }', ["Alice", "Bob"], id="iregexp-uppercase-class"),
        pytest.param('{ name: { iregexp: "^[a-b]" } }', ["Alice", "Bob"], id="iregexp-lowercase-class"),
        pytest.param('{ name: { inregexp: "^A" } }', ["Bob", "Charlie", "Tango"], id="inregexp-uppercase"),
        pytest.param('{ name: { inregexp: "^[B-C]" } }', ["Alice", "Tango"], id="inregexp-uppercase-class"),
        pytest.param('{ _not: { name: { iregexp: "^A" } } }', ["Bob", "Charlie", "Tango"], id="not-iregexp"),
        pytest.param('{ _not: { name: { inregexp: "^A" } } }', ["Alice"], id="not-inregexp"),
        pytest.param('{ name: { iregexp: "E$", regexp: "^A" } }', ["Alice"], id="iregexp-and-regexp"),
        pytest.param('{ name: { iregexp: "E$", nregexp: "^A" } }', ["Charlie"], id="iregexp-and-nregexp"),
        pytest.param('{ name: { regexp: "^[a-z]" } }', [], id="regexp-stays-case-sensitive"),
        pytest.param('{ name: { nregexp: "^[a-z]" } }', _ALL_USERS, id="nregexp-stays-case-sensitive"),
    ],
)
async def test_case_insensitive_regexp_ignores_pattern_case(
    dto_filter: str, names: list[str], any_async_query: AnyQueryExecutor
) -> None:
    """Test that iregexp and inregexp match regardless of the case of either the pattern or the value."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    "raw_users",
    [
        pytest.param(
            [
                {"id": 1, "name": "Émile", "group_id": None, "bio": None},
                {"id": 2, "name": "émile", "group_id": None, "bio": "50%_off"},
                {"id": 3, "name": "Bob", "group_id": None, "bio": "Line one\nline two\n"},
                {"id": 4, "name": "Tango", "group_id": None, "bio": "Tango's bio"},
            ],
            id="non-ascii-names",
        )
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ name: { ilike: "é%" } }', ["Émile", "émile"], id="ilike-lowercase"),
        pytest.param('{ name: { ilike: "É%" } }', ["Émile", "émile"], id="ilike-uppercase"),
        pytest.param('{ name: { ilike: "_MILE" } }', ["Émile", "émile"], id="ilike-underscore-wildcard"),
        pytest.param('{ name: { ilike: "é" } }', [], id="ilike-matches-whole-value"),
        pytest.param('{ name: { ilike: "" } }', [], id="ilike-empty"),
        pytest.param('{ name: { nilike: "É%" } }', ["Bob", "Tango"], id="nilike"),
        pytest.param('{ name: { istartswith: "é" } }', ["Émile", "émile"], id="istartswith-lowercase"),
        pytest.param('{ name: { istartswith: "ÉM" } }', ["Émile", "émile"], id="istartswith-uppercase"),
        pytest.param('{ name: { iendswith: "MILE" } }', ["Émile", "émile"], id="iendswith"),
        pytest.param('{ name: { icontains: "É" } }', ["Émile", "émile"], id="icontains-uppercase"),
        pytest.param('{ name: { icontains: "é" } }', ["Émile", "émile"], id="icontains-lowercase"),
        pytest.param('{ name: { icontains: "" } }', ["Bob", "Tango", "Émile", "émile"], id="icontains-empty"),
        pytest.param('{ _not: { name: { ilike: "é%" } } }', ["Bob", "Tango"], id="not-ilike"),
        pytest.param('{ _not: { name: { icontains: "É" } } }', ["Bob", "Tango"], id="not-icontains"),
        pytest.param('{ bio: { ilike: "50%" } }', ["émile"], id="ilike-percent-wildcard"),
        pytest.param('{ bio: { ilike: "5_%_OFF" } }', ["émile"], id="ilike-wildcards-match-literals"),
        pytest.param('{ bio: { ilike: "line%TWO%" } }', ["Bob"], id="ilike-wildcard-spans-newline"),
        pytest.param('{ bio: { ilike: "%two" } }', [], id="ilike-no-match-before-trailing-newline"),
        pytest.param('{ bio: { nilike: "%bio" } }', ["Bob", "émile"], id="nilike-skips-null"),
        pytest.param('{ _not: { bio: { ilike: "%BIO" } } }', ["Bob", "Émile", "émile"], id="not-ilike-null"),
        pytest.param('{ bio: { icontains: "%" } }', ["émile"], id="icontains-literal-percent"),
        pytest.param('{ bio: { icontains: "_" } }', ["émile"], id="icontains-literal-underscore"),
        pytest.param('{ bio: { icontains: "." } }', [], id="icontains-literal-dot"),
        pytest.param('{ bio: { istartswith: "%" } }', [], id="istartswith-literal-percent"),
        pytest.param('{ bio: { iendswith: "%_OFF" } }', ["émile"], id="iendswith-literal-wildcards"),
        pytest.param('{ bio: { iendswith: "TWO" } }', [], id="iendswith-no-match-before-trailing-newline"),
        pytest.param('{ bio: { icontains: "" } }', ["Bob", "Tango", "émile"], id="icontains-empty-skips-null"),
        pytest.param('{ bio: { icontains: "", ilike: null } }', ["Bob", "Tango", "émile"], id="null-ignored"),
    ],
)
async def test_case_insensitive_like_folds_non_ascii(
    dto_filter: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that the case-insensitive LIKE operators ignore the case of non-ASCII letters."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    "raw_users",
    [
        pytest.param(
            [
                {"id": 1, "name": "Alice", "group_id": None, "bio": "a.b (d) [e] +f ^g $h {2} |"},
                {"id": 2, "name": "Bob", "group_id": None, "bio": "axb"},
                {"id": 3, "name": "Charlie", "group_id": None, "bio": "a" * 2000 + "b"},
                {"id": 4, "name": "Tango", "group_id": None, "bio": "a" * 2000},
            ],
            id="regexp-syntax-and-long-bios",
        )
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ bio: { ilike: "a.b" } }', [], id="ilike-literal-dot"),
        pytest.param('{ bio: { ilike: "a_b" } }', ["Bob"], id="ilike-underscore"),
        pytest.param('{ bio: { ilike: "A.B%" } }', ["Alice"], id="ilike-literal-dot-prefix"),
        pytest.param('{ bio: { ilike: "%(D)%" } }', ["Alice"], id="ilike-literal-parentheses"),
        pytest.param('{ bio: { ilike: "%[E]%" } }', ["Alice"], id="ilike-literal-brackets"),
        pytest.param('{ bio: { ilike: "%[ab]%" } }', [], id="ilike-no-character-class"),
        pytest.param('{ bio: { ilike: "%+F ^G $H%" } }', ["Alice"], id="ilike-literal-anchors"),
        pytest.param('{ bio: { ilike: "%{2} |" } }', ["Alice"], id="ilike-literal-quantifier-and-bar"),
        pytest.param('{ bio: { nilike: "%(d)%" } }', ["Bob", "Charlie", "Tango"], id="nilike-literal-parentheses"),
        pytest.param('{ bio: { ilike: "%a%a%a%b" } }', ["Charlie"], id="ilike-wildcards-long-value"),
        pytest.param('{ bio: { ilike: "%A%A%A%A%A%C" } }', [], id="ilike-wildcards-long-value-no-match"),
        pytest.param('{ bio: { nilike: "%a%a%a%b" } }', ["Alice", "Bob", "Tango"], id="nilike-wildcards-long-value"),
    ],
)
async def test_ilike_patterns_on_regexp_syntax_and_long_values(
    dto_filter: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that ilike and nilike read regular expression syntax literally and stay fast on long values."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    "raw_users",
    [
        pytest.param(
            [
                {"id": 1, "name": "Alice", "group_id": None, "bio": "50%"},
                {"id": 2, "name": "Bob", "group_id": None, "bio": "50\\%"},
                {"id": 3, "name": "Charlie", "group_id": None, "bio": "a_b"},
                {"id": 4, "name": "Tango", "group_id": None, "bio": "axb"},
            ],
            id="escaped-wildcards",
        )
    ],
)
@pytest.mark.parametrize(
    ("operator", "value", "names"),
    [
        pytest.param("like", r"50\%", ["Alice"], id="like-escaped-percent"),
        pytest.param("ilike", r"50\%", ["Alice"], id="ilike-escaped-percent"),
        pytest.param("nlike", r"50\%", ["Bob", "Charlie", "Tango"], id="nlike-escaped-percent"),
        pytest.param("nilike", r"50\%", ["Bob", "Charlie", "Tango"], id="nilike-escaped-percent"),
        pytest.param("like", r"50\\%", ["Bob"], id="like-escaped-backslash"),
        pytest.param("ilike", r"50\\%", ["Bob"], id="ilike-escaped-backslash"),
        pytest.param("nlike", r"50\\%", ["Alice", "Charlie", "Tango"], id="nlike-escaped-backslash"),
        pytest.param("nilike", r"50\\%", ["Alice", "Charlie", "Tango"], id="nilike-escaped-backslash"),
        pytest.param("like", r"a\_b", ["Charlie"], id="like-escaped-underscore"),
        pytest.param("ilike", r"A\_B", ["Charlie"], id="ilike-escaped-underscore"),
        pytest.param("nlike", r"a\_b", ["Alice", "Bob", "Tango"], id="nlike-escaped-underscore"),
        pytest.param("nilike", r"A\_B", ["Alice", "Bob", "Tango"], id="nilike-escaped-underscore"),
        pytest.param("like", r"\a\x\b", ["Tango"], id="like-escaped-literal"),
        pytest.param("ilike", r"\A_\B", ["Charlie", "Tango"], id="ilike-escaped-literal"),
        pytest.param("like", r"%\\%", ["Bob"], id="like-escaped-backslash-inside"),
        pytest.param("like", r"50\%%", ["Alice"], id="like-escaped-then-wildcard"),
        pytest.param("startswith", "50%", ["Alice"], id="startswith-literal-percent"),
        pytest.param("startswith", "50\\", ["Bob"], id="startswith-literal-backslash"),
        pytest.param("endswith", r"\%", ["Bob"], id="endswith-literal-backslash"),
        pytest.param("contains", "_", ["Charlie"], id="contains-literal-underscore"),
        pytest.param("contains", "\\", ["Bob"], id="contains-literal-backslash"),
        pytest.param("icontains", r"\%", ["Bob"], id="icontains-literal-backslash"),
        pytest.param("istartswith", r"50\\", [], id="istartswith-literal-backslashes"),
        pytest.param("iendswith", "_B", ["Charlie"], id="iendswith-literal-underscore"),
    ],
)
async def test_like_backslash_escapes_next_character(
    operator: str,
    value: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that a backslash escapes the next LIKE pattern character, and that the literal operators read it as is."""
    result = await maybe_async(
        any_async_query(f"{{ users(filter: {{ bio: {{ {operator}: {json.dumps(value)} }} }}) {{ name }} }}")
    )
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names


@pytest.mark.parametrize(
    "raw_users",
    [
        pytest.param(
            [
                {"id": 1, "name": "Émile", "group_id": None, "bio": "ABC"},
                {"id": 2, "name": "émile", "group_id": None, "bio": "abc"},
                {"id": 3, "name": "Bob", "group_id": None, "bio": "ABC "},
                {"id": 4, "name": "Tango", "group_id": None, "bio": None},
            ],
            id="mixed-case",
        )
    ],
)
@pytest.mark.parametrize(
    ("dto_filter", "names"),
    [
        pytest.param('{ bio: { like: "ABC" } }', ["Émile"], id="like"),
        pytest.param('{ bio: { like: "abc" } }', ["émile"], id="like-lowercase"),
        pytest.param('{ bio: { like: "a%c" } }', ["émile"], id="like-wildcard"),
        pytest.param('{ bio: { like: "A_C" } }', ["Émile"], id="like-underscore"),
        pytest.param('{ bio: { nlike: "abc" } }', ["Bob", "Émile"], id="nlike"),
        pytest.param('{ bio: { startswith: "ab" } }', ["émile"], id="startswith"),
        pytest.param('{ bio: { endswith: "bc" } }', ["émile"], id="endswith"),
        pytest.param('{ bio: { contains: "b" } }', ["émile"], id="contains"),
        pytest.param('{ bio: { contains: "B" } }', ["Bob", "Émile"], id="contains-uppercase"),
        pytest.param('{ name: { like: "é%" } }', ["émile"], id="like-non-ascii"),
        pytest.param('{ name: { startswith: "É" } }', ["Émile"], id="startswith-non-ascii"),
        pytest.param('{ name: { like: "_mile" } }', ["Émile", "émile"], id="like-underscore-non-ascii"),
        pytest.param('{ name: { like: "emile" } }', [], id="like-accent"),
        pytest.param('{ bio: { ilike: "abc" } }', ["Émile", "émile"], id="ilike"),
        pytest.param('{ bio: { nilike: "abc" } }', ["Bob"], id="nilike"),
        pytest.param('{ bio: { istartswith: "AB" } }', ["Bob", "Émile", "émile"], id="istartswith"),
        pytest.param('{ bio: { iendswith: "BC" } }', ["Émile", "émile"], id="iendswith"),
        pytest.param('{ bio: { icontains: "B" } }', ["Bob", "Émile", "émile"], id="icontains"),
        pytest.param('{ name: { ilike: "emile" } }', [], id="ilike-accent"),
        pytest.param('{ name: { icontains: "EMI" } }', [], id="icontains-accent"),
        pytest.param('{ bio: { like: "ABC " } }', ["Bob"], id="like-trailing-space"),
        pytest.param('{ bio: { endswith: "C" } }', ["Émile"], id="endswith-trailing-space"),
    ],
)
async def test_case_sensitive_like_operators(
    dto_filter: str,
    names: list[str],
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,  # noqa: ARG001
) -> None:
    """Test that like, nlike, startswith, endswith and contains compare letter case, and trailing spaces, exactly."""
    result = await maybe_async(any_async_query(f"{{ users(filter: {dto_filter}) {{ name }} }}"))
    assert not result.errors
    assert result.data is not None

    assert sorted(user["name"] for user in result.data["users"]) == names

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.integration.utils import compute_aggregation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from decimal import Decimal

    from syrupy.assertion import SnapshotAssertion

    from strawchemy.config.databases import DatabaseFeatures

pytestmark = [pytest.mark.integration]


@pytest.mark.parametrize("order_by", ["ASC", "DESC"])
@pytest.mark.snapshot
async def test_order_by(
    order_by: Literal["ASC", "DESC"],
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(
        any_async_query(f"{{ fruits(orderBy: {{ sweetness: {order_by} }}) {{ id sweetness }} }}")
    )
    assert not result.errors
    assert result.data
    # Sort records
    expected_sort = [{"id": row["id"], "sweetness": row["sweetness"]} for row in raw_fruits]
    expected_sort = sorted(expected_sort, key=lambda x: x["sweetness"], reverse=order_by == "DESC")
    assert [{"id": row["id"], "sweetness": row["sweetness"]} for row in result.data["fruits"]] == expected_sort
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("order_by", ["ASC_NULLS_FIRST", "ASC_NULLS_LAST", "DESC_NULLS_FIRST", "DESC_NULLS_LAST"])
@pytest.mark.snapshot
async def test_nulls(
    order_by: Literal["ASC_NULLS_FIRST", "ASC_NULLS_LAST", "DESC_NULLS_FIRST", "DESC_NULLS_LAST"],
    any_async_query: AnyQueryExecutor,
    raw_users: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    result = await maybe_async(any_async_query(f"{{ users(orderBy: {{ bio: {order_by} }}) {{ id bio }} }}"))
    assert not result.errors
    assert result.data
    # Sort records
    expected_sort = [{"id": row["id"], "bio": row["bio"]} for row in raw_users if row["bio"] is not None]
    nulls = [{"id": row["id"], "bio": row["bio"]} for row in raw_users if row["bio"] is None]
    expected_sort = sorted(expected_sort, key=lambda x: x["bio"], reverse=order_by.startswith("DESC"))
    expected_sort = expected_sort + nulls if order_by.endswith("LAST") else nulls + expected_sort
    actual_sort = [{"id": row["id"], "bio": row["bio"]} for row in result.data["users"]]
    assert actual_sort == expected_sort
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "aggregation",
    ["max", "min", "sum", "avg", "stddevSamp", "stddevPop", "varSamp", "varPop"],
)
@pytest.mark.parametrize("order_by", ["ASC", "DESC"])
@pytest.mark.snapshot
async def test_order_by_aggregations(
    order_by: Literal["ASC", "DESC"],
    aggregation: Literal["max", "min", "sum", "avg", "varPop", "stddevPop", "varSamp", "stddevSamp"],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    db_features: DatabaseFeatures,
) -> None:
    if aggregation not in db_features.aggregation_functions:
        pytest.skip(f"{db_features.dialect} does not support {aggregation} aggregation function")

    result = await maybe_async(
        any_async_query(
            f"""{{
            colors(orderBy: {{ fruitsAggregate: {{ {aggregation}: {{ waterPercent: {order_by} }} }} }}) {{
                id
                fruits {{
                    waterPercent
                }}
            }}
        }}"""
        )
    )
    assert not result.errors
    assert result.data

    water_percent_map: dict[str, float | Decimal] = {
        color["id"]: compute_aggregation(
            aggregation, [fruit["water_percent"] for fruit in raw_fruits if fruit["color_id"] == color["id"]]
        )
        for color in raw_colors
    }
    expected_order = sorted(water_percent_map, key=lambda id_: water_percent_map[id_], reverse=order_by == "DESC")
    assert [row["id"] for row in result.data["colors"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("order_by", ["ASC", "DESC"])
@pytest.mark.snapshot
async def test_relation_order_by(
    order_by: Literal["ASC", "DESC"],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            f"""{{
            colors {{
                id
                fruits(orderBy: {{ sweetness: {order_by} }}) {{
                    sweetness
                }}
            }}
        }}"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        expected_order = sorted(
            (fruit["sweetness"] for fruit in raw_fruits if fruit["color_id"] == color["id"]), reverse=order_by == "DESC"
        )
        assert [row["sweetness"] for row in color["fruits"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("order_by", ["ASC", "DESC"])
@pytest.mark.snapshot
async def test_relation_order_by_unselected_column(
    order_by: Literal["ASC", "DESC"],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            f"""{{
            colors {{
                id
                fruits(orderBy: {{ sweetness: {order_by} }}) {{
                    name
                }}
            }}
        }}"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        expected_order = [
            fruit["name"]
            for fruit in sorted(
                (fruit for fruit in raw_fruits if fruit["color_id"] == color["id"]),
                key=lambda fruit: fruit["sweetness"],
                reverse=order_by == "DESC",
            )
        ]
        assert [row["name"] for row in color["fruits"]] == expected_order
        assert all(set(row) == {"name"} for row in color["fruits"])

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


def _color_fruits(raw_fruits: RawRecordData, color_id: int) -> RawRecordData:
    return [fruit for fruit in raw_fruits if fruit["color_id"] == color_id]


def _farm_count(raw_farms: RawRecordData, fruit_id: int) -> int:
    return sum(1 for farm in raw_farms if farm["fruit_id"] == fruit_id)


@pytest.mark.snapshot
async def test_relation_order_by_nested_relation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            """{
            colors {
                id
                fruits(orderBy: [{ color: { name: ASC } }, { sweetness: DESC }]) {
                    name
                    sweetness
                }
            }
        }"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        expected_order = [
            fruit["name"]
            for fruit in sorted(_color_fruits(raw_fruits, color["id"]), key=lambda fruit: -fruit["sweetness"])
        ]
        assert [row["name"] for row in color["fruits"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("order_by", ["ASC", "DESC"])
@pytest.mark.snapshot
async def test_relation_order_by_nested_aggregation(
    order_by: Literal["ASC", "DESC"],
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            f"""{{
            colors {{
                id
                fruits(orderBy: [{{ farmsAggregate: {{ count: {order_by} }} }}, {{ sweetness: ASC }}]) {{
                    name
                    sweetness
                }}
            }}
        }}"""
        )
    )
    assert not result.errors
    assert result.data

    direction = -1 if order_by == "DESC" else 1
    for color in result.data["colors"]:
        expected_order = [
            fruit["name"]
            for fruit in sorted(
                _color_fruits(raw_fruits, color["id"]),
                key=lambda fruit: (direction * _farm_count(raw_farms, fruit["id"]), fruit["sweetness"]),
            )
        ]
        assert [row["name"] for row in color["fruits"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_relation_order_by_nested_relation_aggregation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            """{
            colors {
                id
                fruits(orderBy: [{ color: { fruitsAggregate: { count: ASC } } }, { sweetness: DESC }]) {
                    name
                    sweetness
                }
            }
        }"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        expected_order = [
            fruit["name"]
            for fruit in sorted(_color_fruits(raw_fruits, color["id"]), key=lambda fruit: -fruit["sweetness"])
        ]
        assert [row["name"] for row in color["fruits"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_paginated_relation_order_by_nested_aggregation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            """{
            colorsPaginated {
                id
                fruits(limit: 1, orderBy: [{ farmsAggregate: { count: DESC } }, { sweetness: ASC }]) {
                    name
                    sweetness
                }
            }
        }"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colorsPaginated"]:
        expected_order = sorted(
            _color_fruits(raw_fruits, color["id"]),
            key=lambda fruit: (-_farm_count(raw_farms, fruit["id"]), fruit["sweetness"]),
        )
        assert [row["name"] for row in color["fruits"]] == [expected_order[0]["name"]]

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_paginated_relation_order_by_nested_relation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            """{
            colorsPaginated {
                id
                fruits(limit: 1, offset: 1, orderBy: [{ color: { name: ASC } }, { sweetness: DESC }]) {
                    name
                    sweetness
                }
            }
        }"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colorsPaginated"]:
        expected_order = sorted(_color_fruits(raw_fruits, color["id"]), key=lambda fruit: -fruit["sweetness"])
        assert [row["name"] for row in color["fruits"]] == [expected_order[1]["name"]]

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_distinct_on_with_relation_order_by_nested_aggregation(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    result = await maybe_async(
        any_async_query(
            """{
            colors(distinctOn: [name], orderBy: [{ name: ASC }]) {
                id
                fruits(orderBy: [{ farmsAggregate: { count: ASC } }, { sweetness: DESC }]) {
                    name
                    sweetness
                }
            }
        }"""
        )
    )
    assert not result.errors
    assert result.data

    for color in result.data["colors"]:
        expected_order = [
            fruit["name"]
            for fruit in sorted(
                _color_fruits(raw_fruits, color["id"]),
                key=lambda fruit: (_farm_count(raw_farms, fruit["id"]), -fruit["sweetness"]),
            )
        ]
        assert [row["name"] for row in color["fruits"]] == expected_order

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_deterministic_ordering(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that list resolvers return ordered results even if no order by is specified."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors {
                    id
                    fruits {
                        id
                    }
                }
            }
        """
        )
    )
    assert not result.errors
    assert result.data

    assert result.data["colors"] == [
        {"id": 1, "fruits": [{"id": 1}, {"id": 2}]},
        {"id": 2, "fruits": [{"id": 3}, {"id": 4}, {"id": 5}]},
        {"id": 3, "fruits": [{"id": 6}, {"id": 7}]},
        {"id": 4, "fruits": [{"id": 8}, {"id": 9}]},
        {"id": 5, "fruits": [{"id": 10}, {"id": 11}]},
    ]
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_deterministic_ordering_mixed_with_user_ordering(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that list resolvers return ordered results even if no order by is specified."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors(orderBy: { name: ASC }) {
                    id
                    fruits {
                        id
                    }
                }
            }
        """
        )
    )
    assert not result.errors
    assert result.data

    assert result.data["colors"] == [
        {"id": 4, "fruits": [{"id": 8}, {"id": 9}]},
        {"id": 3, "fruits": [{"id": 6}, {"id": 7}]},
        {"id": 5, "fruits": [{"id": 10}, {"id": 11}]},
        {"id": 1, "fruits": [{"id": 1}, {"id": 2}]},
        {"id": 2, "fruits": [{"id": 3}, {"id": 4}, {"id": 5}]},
    ]
    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


def _default_colors(raw_colors: RawRecordData, raw_fruits: RawRecordData) -> list[dict[str, Any]]:
    return [
        {"id": color["id"], "fruits": [{"id": fruit["id"]} for fruit in _color_fruits(raw_fruits, color["id"])]}
        for color in sorted(raw_colors, key=lambda color: color["id"])
    ]


@pytest.mark.parametrize(
    ("query", "variables"),
    [
        pytest.param("{ colors(orderBy: {}) { id fruits { id } } }", None, id="root-empty"),
        pytest.param("{ colors(orderBy: [{}]) { id fruits { id } } }", None, id="root-empty-list"),
        pytest.param(
            "query ($d: OrderByEnum) { colors(orderBy: { name: $d }) { id fruits { id } } }",
            {},
            id="root-omitted-variable",
        ),
        pytest.param("{ colors { id fruits(orderBy: {}) { id } } }", None, id="nested-empty"),
        pytest.param("{ colors { id fruits(orderBy: [{}]) { id } } }", None, id="nested-empty-list"),
        pytest.param(
            "query ($d: OrderByEnum) { colors { id fruits(orderBy: { sweetness: $d }) { id } } }",
            {},
            id="nested-omitted-variable",
        ),
        pytest.param("{ colors { id fruits(orderBy: { color: {} }) { id } } }", None, id="nested-empty-relation"),
        pytest.param(
            "{ colors { id fruits(orderBy: { color: { fruits: {} } }) { id } } }",
            None,
            id="nested-empty-nested-relation",
        ),
        pytest.param("{ colors(orderBy: { name: null }) { id fruits { id } } }", None, id="root-null"),
        pytest.param("{ colors(orderBy: null) { id fruits { id } } }", None, id="root-null-argument"),
        pytest.param("{ colors { id fruits(orderBy: { color: null }) { id } } }", None, id="nested-null-relation"),
        pytest.param(
            "{ colors { id fruits(orderBy: { color: { name: null } }) { id } } }",
            None,
            id="nested-null-in-relation",
        ),
        pytest.param(
            "query ($d: OrderByEnum) { colors(orderBy: { name: $d }) { id fruits { id } } }",
            {"d": None},
            id="root-null-variable",
        ),
    ],
)
async def test_empty_order_by_is_ignored(
    query: str,
    variables: dict[str, Any] | None,
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    result = await maybe_async(any_async_query(query, variables))
    assert not result.errors
    assert result.data
    assert result.data["colors"] == _default_colors(raw_colors, raw_fruits)


@pytest.mark.parametrize(
    ("empty_entry", "variables"),
    [
        pytest.param("{}", None, id="empty"),
        pytest.param("{ color: {} }", None, id="empty-relation"),
        pytest.param("{ color: { fruits: {} } }", None, id="empty-nested-relation"),
        pytest.param("{ color: { name: $d } }", {}, id="omitted-variable-in-relation"),
        pytest.param("{ color: null, name: null }", None, id="null"),
        pytest.param("{ color: { name: $d } }", {"d": None}, id="null-variable-in-relation"),
    ],
)
async def test_empty_order_by_entry_keeps_other_entries(
    empty_entry: str,
    variables: dict[str, Any] | None,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
) -> None:
    signature = "query ($d: OrderByEnum) " if variables is not None else ""
    result = await maybe_async(
        any_async_query(
            f"{signature}{{ fruits(orderBy: [{{ color: {{}}, sweetness: ASC }}, {empty_entry}, {{ id: DESC }}]) {{ id }} }}",
            variables,
        )
    )
    assert not result.errors
    assert result.data
    expected = [fruit["id"] for fruit in sorted(raw_fruits, key=lambda fruit: (fruit["sweetness"], -fruit["id"]))]
    assert [row["id"] for row in result.data["fruits"]] == expected


@pytest.mark.parametrize(
    ("empty_entry", "variables"),
    [
        pytest.param("{}", None, id="empty"),
        pytest.param("{ fruitsAggregate: {} }", None, id="empty-aggregate"),
        pytest.param("{ fruitsAggregate: { max: {} } }", None, id="empty-aggregate-function"),
        pytest.param("{ fruitsAggregate: { count: $d } }", {}, id="omitted-variable-in-aggregate"),
    ],
)
async def test_empty_order_by_entry_keeps_other_entries_nested(
    empty_entry: str,
    variables: dict[str, Any] | None,
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
) -> None:
    signature = "query ($d: OrderByEnum) " if variables is not None else ""
    result = await maybe_async(
        any_async_query(
            f"""{signature}{{
            colors(orderBy: [{empty_entry}, {{ id: DESC }}]) {{
                id
                fruits(orderBy: [{{}}, {{ color: {{ fruits: {{}} }}, sweetness: DESC }}, {{ color: {{}} }}, {{ id: DESC }}]) {{
                    id
                }}
            }}
        }}""",
            variables,
        )
    )
    assert not result.errors
    assert result.data
    expected = [
        {
            "id": color["id"],
            "fruits": [
                {"id": fruit["id"]}
                for fruit in sorted(
                    _color_fruits(raw_fruits, color["id"]), key=lambda fruit: (-fruit["sweetness"], -fruit["id"])
                )
            ],
        }
        for color in sorted(raw_colors, key=lambda color: -color["id"])
    ]
    assert result.data["colors"] == expected

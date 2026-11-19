from __future__ import annotations

import re
from collections import Counter
from typing import TYPE_CHECKING, Any, Literal

import pytest

from strawchemy.schema.pagination import DefaultOffsetPagination
from tests.integration.fixtures import QueryTracker
from tests.integration.models import Fruit
from tests.integration.typing import RawRecordData
from tests.integration.utils import compute_aggregation, from_graphql_representation, python_type
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.config.databases import DatabaseFeatures

pytestmark = [pytest.mark.integration]


@pytest.mark.snapshot
async def test_count_aggregation(
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the count aggregation function."""
    query = f"""
        {{
            color(id: {raw_colors[0]["id"]}) {{
                fruitsAggregate {{
                    count
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert result.data["color"]["fruitsAggregate"]["count"] == len(
        [fruit for fruit in raw_fruits if fruit["color_id"] == raw_colors[0]["id"]]
    )
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_aggregation_without_related_rows(any_async_query: AnyQueryExecutor) -> None:
    """Test that a parent with no related rows is returned with a zero count and null extrema."""
    created = await maybe_async(any_async_query('mutation { createColor(data: { name: "Blue" }) { id } }'))
    assert not created.errors

    result = await maybe_async(
        any_async_query(
            """
            {
                colors(filter: { name: { eq: "Blue" } }) {
                    name
                    fruitsAggregate {
                        count
                        max { sweetness }
                    }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    assert result.data["colors"] == [{"name": "Blue", "fruitsAggregate": {"count": 0, "max": {"sweetness": None}}}]


async def test_count_aggregation_many_to_many(
    any_async_query: AnyQueryExecutor, raw_user_departments: RawRecordData, raw_users: RawRecordData
) -> None:
    """Test that a many-to-many count matches the number of join-table rows of each user."""
    result = await maybe_async(
        any_async_query(
            """
            {
                users {
                    id
                    departmentsAggregate { count }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    departments_per_user = Counter(link["user_id"] for link in raw_user_departments)
    actual = {user["id"]: user["departmentsAggregate"]["count"] for user in result.data["users"]}
    assert actual == {user["id"]: departments_per_user[user["id"]] for user in raw_users}


async def test_count_aggregation_many_to_many_join_predicates(
    any_async_query: AnyQueryExecutor,
    raw_user_departments: RawRecordData,
    raw_users: RawRecordData,
    raw_departments: RawRecordData,
) -> None:
    """Test that a many-to-many count honours the relationship primaryjoin and secondaryjoin predicates."""
    result = await maybe_async(
        any_async_query(
            """
            {
                users {
                    id
                    filteredDepartmentsAggregate { count }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    user_names = {user["id"]: user["name"] for user in raw_users}
    department_names = {department["id"]: department["name"] for department in raw_departments}
    joined = Counter(
        link["user_id"]
        for link in raw_user_departments
        if user_names[link["user_id"]] != "Bob" and department_names[link["department_id"]] != "IT"
    )
    actual = {user["id"]: user["filteredDepartmentsAggregate"]["count"] for user in result.data["users"]}
    assert actual == {user["id"]: joined[user["id"]] for user in raw_users}


async def test_nested_aggregation_correlates_to_its_own_element(
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
) -> None:
    """Test that an aggregate nested under a to-many relation reports the value of its own element."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors {
                    id
                    fruits {
                        id
                        farmsAggregate { max { id } }
                    }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    assert [color["id"] for color in result.data["colors"]] == [color["id"] for color in raw_colors]
    actual = {
        fruit["id"]: fruit["farmsAggregate"]["max"]["id"]
        for color in result.data["colors"]
        for fruit in color["fruits"]
    }
    assert actual == {
        fruit["id"]: max(farm["id"] for farm in raw_farms if farm["fruit_id"] == fruit["id"]) for fruit in raw_fruits
    }


async def test_count_aggregation_many_to_many_nested(
    any_async_query: AnyQueryExecutor, raw_user_departments: RawRecordData
) -> None:
    """Test that a nested many-to-many count matches the number of users of each department."""
    result = await maybe_async(
        any_async_query(
            """
            {
                users {
                    id
                    departments {
                        id
                        usersAggregate { count }
                    }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    users_per_department = Counter(link["department_id"] for link in raw_user_departments)
    actual = {
        (user["id"], department["id"]): department["usersAggregate"]["count"]
        for user in result.data["users"]
        for department in user["departments"]
    }
    assert actual == {
        (link["user_id"], link["department_id"]): users_per_department[link["department_id"]]
        for link in raw_user_departments
    }


@pytest.mark.parametrize(
    ("field_name", "raw_field_name"),
    [
        ("sweetness", "sweetness"),
        ("waterPercent", "water_percent"),
    ],
)
@pytest.mark.snapshot
async def test_sum_aggregation(
    field_name: str,
    raw_field_name: str,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the sum aggregation function for a specific field."""
    query = f"""
        {{
            color(id: {raw_colors[0]["id"]}) {{
                fruitsAggregate {{
                    sum {{
                        {field_name}
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    # Calculate expected value
    expected_sum = sum(fruit[raw_field_name] for fruit in raw_fruits if fruit["color_id"] == raw_colors[0]["id"])

    # Verify result
    actual_sum = result.data["color"]["fruitsAggregate"]["sum"][field_name]

    assert pytest.approx(actual_sum) == expected_sum

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field_name", "raw_field_name"),
    [
        ("sweetness", "sweetness"),
        ("waterPercent", "water_percent"),
        ("name", "name"),
        ("createdAt", "created_at"),
        ("bestTimeToPick", "best_time_to_pick"),
    ],
)
@pytest.mark.snapshot
async def test_min_aggregation(
    field_name: str,
    raw_field_name: str,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the min aggregation function for a specific field."""
    query = f"""
        {{
            color(id: {raw_colors[0]["id"]}) {{
                fruitsAggregate {{
                    min {{
                        {field_name}
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    # Verify result
    actual_min = from_graphql_representation(
        result.data["color"]["fruitsAggregate"]["min"][field_name], python_type(Fruit, raw_field_name)
    )
    assert actual_min is not None

    # For fields where we can calculate expected values, verify them
    expected_min = min(fruit[raw_field_name] for fruit in raw_fruits if fruit["color_id"] == raw_colors[0]["id"])

    assert actual_min == expected_min

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field_name", "raw_field_name"),
    [
        ("sweetness", "sweetness"),
        ("waterPercent", "water_percent"),
        ("name", "name"),
        ("createdAt", "created_at"),
        ("bestTimeToPick", "best_time_to_pick"),
    ],
)
@pytest.mark.snapshot
async def test_max_aggregation(
    field_name: str,
    raw_field_name: str,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test the max aggregation function for a specific field."""
    query = f"""
        {{
            color(id: {raw_colors[0]["id"]}) {{
                fruitsAggregate {{
                    max {{
                        {field_name}
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    # Verify result
    actual_max = from_graphql_representation(
        result.data["color"]["fruitsAggregate"]["max"][field_name], python_type(Fruit, raw_field_name)
    )
    assert actual_max is not None

    # For fields where we can calculate expected values, verify them
    expected_max = max(fruit[raw_field_name] for fruit in raw_fruits if fruit["color_id"] == raw_colors[0]["id"])

    assert actual_max == expected_max

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "agg_type",
    ["avg", "stddevSamp", "stddevPop", "varSamp", "varPop"],
)
@pytest.mark.parametrize(
    ("field_name", "raw_field_name"),
    [
        ("sweetness", "sweetness"),
        ("waterPercent", "water_percent"),
    ],
)
@pytest.mark.snapshot
async def test_statistical_aggregation(
    agg_type: Literal["avg", "stddevSamp", "stddevPop", "varSamp", "varPop"],
    field_name: str,
    raw_field_name: str,
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    db_features: DatabaseFeatures,
) -> None:
    """Test statistical aggregation functions for a specific field."""
    if agg_type not in db_features.aggregation_functions:
        pytest.skip(f"{db_features.dialect} does not support {agg_type} aggregation function")

    query = f"""
        {{
            color(id: {raw_colors[0]["id"]}) {{
                fruitsAggregate {{
                    {agg_type} {{
                        {field_name}
                    }}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    # Verify result is a number or null
    actual_value = from_graphql_representation(
        result.data["color"]["fruitsAggregate"][agg_type][field_name], python_type(Fruit, raw_field_name)
    )

    expected_value = compute_aggregation(
        agg_type, [fruit[raw_field_name] for fruit in raw_fruits if fruit["color_id"] == raw_colors[0]["id"]]
    )

    assert pytest.approx(actual_value) == expected_value

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "pagination",
    [pytest.param(None, id="no-pagination"), pytest.param(DefaultOffsetPagination(limit=2), id="pagination")],
)
@pytest.mark.parametrize("agg_type", ["sum", "avg", "stddevSamp", "stddevPop", "varSamp", "varPop"])
@pytest.mark.parametrize(
    ("field_name", "raw_field_name"),
    [
        ("sweetness", "sweetness"),
        ("waterPercent", "water_percent"),
    ],
)
@pytest.mark.snapshot
async def test_root_aggregation(
    agg_type: Literal["sum", "avg", "stddevSamp", "stddevPop", "varSamp", "varPop"],
    field_name: str,
    raw_field_name: str,
    pagination: DefaultOffsetPagination | None,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
    db_features: DatabaseFeatures,
) -> None:
    """Test statistical aggregation functions for a specific field."""
    if agg_type not in db_features.aggregation_functions:
        pytest.skip(f"{db_features.dialect} does not support {agg_type} aggregation function")

    query_name = "fruitAggregations" if pagination is None else "fruitAggregationsPaginatedLimit2"
    query = f"""
        {{
            {query_name} {{
                aggregations {{
                    {agg_type} {{
                        {field_name}
                    }}
                }}
                nodes {{
                    id
                    {field_name}
                }}
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    # Verify result is a number or null
    actual_value = from_graphql_representation(
        result.data[query_name]["aggregations"][agg_type][field_name], python_type(Fruit, raw_field_name)
    )
    if pagination is None:
        expected_value = compute_aggregation(agg_type, [record[raw_field_name] for record in raw_fruits])
    else:
        expected_value = compute_aggregation(
            agg_type, [record[raw_field_name] for record in raw_fruits[: pagination.limit]]
        )

    rel = (
        0.0001 if db_features.dialect == "mysql" and field_name == "sweetness" and agg_type in ("avg", "sum") else None
    )
    assert actual_value == pytest.approx(expected_value, rel=rel)

    # Verify SQL query
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def _data(any_async_query: AnyQueryExecutor, query: str) -> dict[str, Any]:
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    return result.data


def _farms_aggregate(raw_farms: RawRecordData, fruit_id: int) -> dict[str, Any]:
    names = [farm["name"] for farm in raw_farms if farm["fruit_id"] == fruit_id]
    return {"count": len(names), "min": {"name": min(names)}, "max": {"name": max(names)}}


@pytest.mark.parametrize(
    ("root", "arguments"),
    [
        pytest.param("colors", "(orderBy: { sweetness: ASC })", id="order-by"),
        pytest.param("colorsPaginated", "(limit: 10, offset: 0)", id="limit-offset"),
        pytest.param("colorsPaginated", "", id="default-pagination"),
    ],
)
async def test_nested_aggregation_under_relation_with_arguments(
    root: str,
    arguments: str,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
    query_tracker: QueryTracker,
) -> None:
    """Test that an aggregate under a relation that gets its own subquery is computed per related row."""
    data = await _data(
        any_async_query,
        f"""
        {{
            {root} {{
                id
                fruits{arguments} {{ id farmsAggregate {{ count min {{ name }} max {{ name }} }} }}
            }}
        }}
        """,
    )
    fruits = [fruit for color in data[root] for fruit in color["fruits"]]
    assert sorted(fruit["id"] for fruit in fruits) == sorted(
        fruit["id"] for fruit in raw_fruits if fruit["color_id"] is not None
    )
    for fruit in fruits:
        assert fruit["farmsAggregate"] == _farms_aggregate(raw_farms, fruit["id"])
    assert query_tracker.query_count == 1


async def test_nested_aggregation_under_relation_with_distinct_on(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test that an aggregate under a relation with DISTINCT ON is computed per kept row."""
    data = await _data(
        any_async_query,
        """
        {
            colorsNestedDistinct {
                id
                fruits(distinctOn: [sweetness]) { id sweetness farmsAggregate { count min { name } max { name } } }
            }
        }
        """,
    )
    for color in data["colorsNestedDistinct"]:
        sweetness = {fruit["sweetness"] for fruit in raw_fruits if fruit["color_id"] == color["id"]}
        assert sorted(fruit["sweetness"] for fruit in color["fruits"]) == sorted(sweetness)
        for fruit in color["fruits"]:
            assert fruit["farmsAggregate"] == _farms_aggregate(raw_farms, fruit["id"])
    assert query_tracker.query_count == 1


_ROOT_AGGREGATION_NEXT_TO_NESTED = """
    {{
        colors{arguments} {{
            id
            fruitsAggregate {{ count }}
            fruits(orderBy: {{ sweetness: ASC }}) {{ id farmsAggregate {{ count }} }}
        }}
    }}
"""


def _assert_nested_aggregates(
    colors: list[dict[str, Any]], raw_fruits: RawRecordData, raw_farms: RawRecordData
) -> None:
    fruit_counts = Counter(fruit["color_id"] for fruit in raw_fruits)
    for color in colors:
        fruits = sorted(
            (fruit for fruit in raw_fruits if fruit["color_id"] == color["id"]), key=lambda fruit: fruit["sweetness"]
        )
        assert color["fruitsAggregate"] == {"count": fruit_counts[color["id"]]}
        assert color["fruits"] == [
            {"id": fruit["id"], "farmsAggregate": {"count": _farms_aggregate(raw_farms, fruit["id"])["count"]}}
            for fruit in fruits
        ]


async def test_root_aggregation_filter_next_to_nested_aggregation(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test a root aggregation filter next to an aggregate under an ordered relation."""
    data = await _data(
        any_async_query,
        _ROOT_AGGREGATION_NEXT_TO_NESTED.format(
            arguments="(filter: { fruitsAggregate: { count: { predicate: { gt: 1 } } } })"
        ),
    )
    fruit_counts = Counter(fruit["color_id"] for fruit in raw_fruits)
    assert {color["id"] for color in data["colors"]} == {
        color_id for color_id, count in fruit_counts.items() if count > 1
    }
    _assert_nested_aggregates(data["colors"], raw_fruits, raw_farms)
    assert query_tracker.query_count == 1


async def test_root_aggregation_order_by_next_to_nested_aggregation(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test a root aggregation ordering next to an aggregate under an ordered relation."""
    data = await _data(
        any_async_query,
        _ROOT_AGGREGATION_NEXT_TO_NESTED.format(arguments="(orderBy: { fruitsAggregate: { count: DESC } })"),
    )
    counts = [color["fruitsAggregate"]["count"] for color in data["colors"]]
    assert counts == sorted(counts, reverse=True)
    _assert_nested_aggregates(data["colors"], raw_fruits, raw_farms)
    assert query_tracker.query_count == 1


@pytest.mark.parametrize(
    ("root", "expected_fruit_ids"),
    [
        pytest.param("colorsWithOrderedFruits", None, id="ordering-hook"),
        pytest.param("colorsWithMultiFarmFruits", {1, 2}, id="joining-hook"),
    ],
)
async def test_nested_aggregation_under_hooked_relation(
    root: str,
    expected_fruit_ids: set[int] | None,
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
    query_tracker: QueryTracker,
) -> None:
    """Test that an aggregate under a relation whose query hook is not WHERE-only is computed per related row."""
    data = await _data(any_async_query, f"{{ {root} {{ id fruits {{ id farmsAggregate {{ count }} }} }} }}")
    fruits = [fruit for color in data[root] for fruit in color["fruits"]]
    colored_fruit_ids = {fruit["id"] for fruit in raw_fruits if fruit["color_id"] is not None}
    assert {fruit["id"] for fruit in fruits} == (expected_fruit_ids or colored_fruit_ids)
    for fruit in fruits:
        assert fruit["farmsAggregate"] == {"count": _farms_aggregate(raw_farms, fruit["id"])["count"]}
    assert query_tracker.query_count == 1


async def test_aggregation_two_levels_under_relation_with_arguments(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test that an aggregate under a to-one relation of an ordered relation reports its own parent's value."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors {
                    id
                    fruits(orderBy: { sweetness: ASC }) {
                        id
                        color {
                            id
                            fruitsAggregate { count sum { sweetness } min { sweetness } max { sweetness } }
                        }
                    }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    for color in result.data["colors"]:
        sweetness = [fruit["sweetness"] for fruit in raw_fruits if fruit["color_id"] == color["id"]]
        expected = {
            "count": len(sweetness),
            "sum": {"sweetness": sum(sweetness)},
            "min": {"sweetness": min(sweetness)},
            "max": {"sweetness": max(sweetness)},
        }
        assert [fruit["color"] for fruit in color["fruits"]] == [
            {"id": color["id"], "fruitsAggregate": expected} for _ in sweetness
        ]
    assert query_tracker.query_count == 1


@pytest.mark.allow_duplicate_reads(
    reason=(
        "on postgresql, colors and colors.fruits.color each compute fruitsAggregate in a LATERAL correlated to "
        "their own alias; the CTE databases share one grouped CTE"
    ),
    dialects=("postgresql",),
)
async def test_same_aggregate_on_root_and_under_relation(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test that one aggregate selected on the root and under a relation reports each parent's value."""
    result = await maybe_async(
        any_async_query(
            "{ colors { id fruitsAggregate { count } fruits { id color { id fruitsAggregate { count } } } } }"
        )
    )
    assert not result.errors
    assert result.data
    for color in result.data["colors"]:
        count = sum(1 for fruit in raw_fruits if fruit["color_id"] == color["id"])
        assert color["fruitsAggregate"] == {"count": count}
        assert [fruit["color"] for fruit in color["fruits"]] == [
            {"id": color["id"], "fruitsAggregate": {"count": count}} for _ in range(count)
        ]
    assert query_tracker.query_count == 1


async def test_aggregate_under_aliases_differing_in_limit(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    raw_farms: RawRecordData,
    query_tracker: QueryTracker,
    db_features: DatabaseFeatures,
) -> None:
    """Test that aliases of a relation differing in limit each count their farms, sharing one grouped CTE without LATERAL."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colorsPaginated {
                    id
                    a: fruits(limit: 1) { id farmsAggregate { count } }
                    b: fruits(limit: 2) { id farmsAggregate { count } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    farm_counts = Counter(farm["fruit_id"] for farm in raw_farms)
    for color in result.data["colorsPaginated"]:
        fruits = [
            {"id": fruit["id"], "farmsAggregate": {"count": farm_counts[fruit["id"]]}}
            for fruit in sorted(raw_fruits, key=lambda fruit: fruit["id"])
            if fruit["color_id"] == color["id"]
        ]
        assert color["a"] == fruits[:1]
        assert color["b"] == fruits[:2]
    assert query_tracker.query_count == 1
    if not db_features.supports_lateral:
        assert len(re.findall(r"\banon_\d+ AS\s*\(", query_tracker[0].statement_str)) == 2


async def test_nested_aggregation_under_aliased_relations_with_arguments(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData, raw_farms: RawRecordData, query_tracker: QueryTracker
) -> None:
    """Test that each alias of an ordered relation computes its own nested aggregate."""
    result = await maybe_async(
        any_async_query(
            """
            {
                colors {
                    id
                    a: fruits(orderBy: { sweetness: ASC }) { id farmsAggregate { count min { name } max { name } } }
                    b: fruits(orderBy: { sweetness: DESC }) { id farmsAggregate { count } }
                }
            }
            """
        )
    )
    assert not result.errors
    assert result.data
    for color in result.data["colors"]:
        fruits = sorted(
            (fruit for fruit in raw_fruits if fruit["color_id"] == color["id"]), key=lambda fruit: fruit["sweetness"]
        )
        assert color["a"] == [
            {"id": fruit["id"], "farmsAggregate": _farms_aggregate(raw_farms, fruit["id"])} for fruit in fruits
        ]
        assert color["b"] == [
            {"id": fruit["id"], "farmsAggregate": {"count": _farms_aggregate(raw_farms, fruit["id"])["count"]}}
            for fruit in reversed(fruits)
        ]
    assert query_tracker.query_count == 1

from __future__ import annotations

import re
from typing import TYPE_CHECKING

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

pytestmark = [pytest.mark.integration]


@pytest.mark.parametrize(
    ("predicate", "value", "expected_indices"),
    [
        pytest.param("eq", 2, [0, 2, 3, 4], id="eq-match"),
        pytest.param("neq", 0, [0, 1, 2, 3, 4], id="neq-match"),
        pytest.param("gt", 1, [0, 1, 2, 3, 4], id="gt-match"),
        pytest.param("gte", 2, [0, 1, 2, 3, 4], id="gte-match"),
        pytest.param("lt", 3, [0, 2, 3, 4], id="lt-match"),
        pytest.param("lte", 2, [0, 2, 3, 4], id="lte-match"),
        pytest.param("in", [1, 2, 3], [0, 1, 2, 3, 4], id="in-match"),
        pytest.param("nin", [0, 3, 4], [0, 2, 3, 4], id="nin-match"),
    ],
)
@pytest.mark.snapshot
async def test_count_aggregation_filter(
    predicate: str,
    value: int | list[int],
    expected_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by count aggregation."""
    # Prepare the value for GraphQL query
    value_str = f"[{', '.join(str(v) for v in value)}]" if isinstance(value, list) else str(value)

    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    count: {{
                        arguments: [id]
                        predicate: {{ {predicate}: {value_str} }}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_indices)

    # Get the container IDs from the result
    result_container_ids = {container["id"] for container in result.data["colors"]}

    # Get the expected container IDs
    expected_container_ids = {raw_colors[idx]["id"] for idx in expected_indices}

    # Assert that the result contains exactly the expected container IDs
    assert result_container_ids == expected_container_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field", "predicate", "value", "expected_color_indices"),
    [
        pytest.param("name", "eq", "Apple", [0], id="eq-match"),
        pytest.param("name", "like", "%pp%", [0], id="like-match"),
        pytest.param("name", "ilike", "%APP%", [0], id="ilike-match"),
        pytest.param("name", "startswith", "App", [0], id="startswith-match"),
        pytest.param("name", "contains", "ppl", [0], id="contains-match"),
    ],
)
@pytest.mark.snapshot
async def test_min_string_aggregation_filter(
    field: str,
    predicate: str,
    value: str,
    expected_color_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by minString aggregation."""
    value_str = f'"{value}"'

    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    minString: {{
                        arguments: [{field}]
                        predicate: {{ {predicate}: {value_str} }}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_color_indices)

    # Get the color IDs from the result
    result_color_ids = {color["id"] for color in result.data["colors"]}

    # Get the expected color IDs
    expected_color_ids = {raw_colors[idx]["id"] for idx in expected_color_indices}

    # Assert that the result contains exactly the expected color IDs
    assert result_color_ids == expected_color_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field", "predicate", "value", "expected_color_indices"),
    [
        pytest.param("name", "eq", "Cherry", [0], id="eq-match"),
        pytest.param("name", "like", "%err%", [0, 3], id="like-match"),
        pytest.param("name", "ilike", "%ERR%", [0, 3], id="ilike-match"),
        pytest.param("name", "startswith", "Che", [0], id="startswith-match"),
        pytest.param("name", "contains", "err", [0, 3], id="contains-match"),
    ],
)
@pytest.mark.snapshot
async def test_max_string_aggregation_filter(
    field: str,
    predicate: str,
    value: str,
    expected_color_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by maxString aggregation."""
    value_str = f'"{value}"'

    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    maxString: {{
                        arguments: [{field}]
                        predicate: {{ {predicate}: {value_str} }}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_color_indices)

    # Get the color IDs from the result
    result_color_ids = {color["id"] for color in result.data["colors"]}

    # Get the expected color IDs
    expected_color_ids = {raw_colors[idx]["id"] for idx in expected_color_indices}

    # Assert that the result contains exactly the expected color IDs
    assert result_color_ids == expected_color_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field", "predicate", "value", "expected_color_indices"),
    [
        pytest.param("sweetness", "eq", 6, [1], id="int-eq-match"),
        pytest.param("sweetness", "gt", 8, [0, 2, 4], id="int-gt-match"),
        pytest.param("waterPercent", "eq", 1.77, [0], id="float-eq-match"),
        pytest.param("waterPercent", "gt", 1.7, [0, 1, 2], id="float-gt-match"),
    ],
)
@pytest.mark.snapshot
async def test_sum_aggregation_filter(
    field: str,
    predicate: str,
    value: float,
    expected_color_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by sum aggregation."""
    value_str = str(value)

    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    sum: {{
                        arguments: [{field}]
                        predicate: {{ {predicate}: {value_str} }}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_color_indices)

    # Get the color IDs from the result
    result_color_ids = {color["id"] for color in result.data["colors"]}

    # Get the expected color IDs
    expected_color_ids = {raw_colors[idx]["id"] for idx in expected_color_indices}

    # Assert that the result contains exactly the expected color IDs
    assert result_color_ids == expected_color_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("field", "predicate", "value", "expected_color_indices"),
    [
        pytest.param("sweetness", "eq", 6.5, [0], id="int-avg-eq-match"),
        pytest.param("sweetness", "gt", 7.0, [2, 4], id="int-avg-gt-match"),
        pytest.param("waterPercent", "eq", 0.885, [0], id="float-avg-eq-match"),
        pytest.param("waterPercent", "gt", 0.85, [0, 2], id="float-avg-gt-match"),
    ],
)
@pytest.mark.snapshot
async def test_avg_aggregation_filter(
    field: str,
    predicate: str,
    value: float,
    expected_color_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by avg aggregation."""
    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    avg: {{
                        arguments: [{field}]
                        predicate: {{ {predicate}: {value} }}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_color_indices)

    # Get the color IDs from the result
    result_color_ids = {color["id"] for color in result.data["colors"]}

    # Get the expected color IDs
    expected_color_ids = {raw_colors[idx]["id"] for idx in expected_color_indices}

    # Assert that the result contains exactly the expected color IDs
    assert result_color_ids == expected_color_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_count_aggregation_filter_nested_under_or(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that an aggregation predicate nested under a top-level ``_or`` is discovered and applied.

    Guards the build-once collect pass: it must walk into ``_or``/``_not`` branches when
    gathering aggregation filters, otherwise the nested predicate is silently dropped. The
    snapshot shows the aggregation subquery is emitted under the ``OR`` branch.
    """
    query = """
        {
            colors(filter: {
                _or: [
                    { fruitsAggregate: { count: { predicate: { gt: 0 } } } }
                ]
            }) {
                id
                fruits {
                    id
                }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    # Every color has at least one fruit, so the count > 0 predicate matches all of them.
    assert result.data["colors"]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_aggregation_built_once_across_filter_and_order_by(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test that the aggregation of a relation is built once when filtering and ordering by it.

    Filtering on one aggregation function and ordering by a *different* function of the same
    relation must reuse a single aggregation subquery; the snapshot shows one aggregation
    join, not two identically named ones.
    """
    query = """
        {
            colors(
                filter: { fruitsAggregate: { count: { predicate: { gt: 0 } } } },
                orderBy: { fruitsAggregate: { sum: { sweetness: ASC } } }
            ) {
                fruits {
                    id
                }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("distinct", "expected_count", "expected_color_indices"),
    [
        pytest.param(True, 2, [0, 2, 3, 4], id="distinct-match"),
        pytest.param(False, 2, [0, 2, 3, 4], id="non-distinct-match"),
        pytest.param(None, 2, [0, 2, 3, 4], id="default-match"),
    ],
)
@pytest.mark.snapshot
async def test_count_aggregation_filter_with_distinct(
    distinct: bool | None,
    expected_count: int,
    expected_color_indices: list[int],
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Test filtering by count aggregation with distinct option."""
    distinct_str = f"distinct: {to_graphql_representation(distinct, 'input')}" if distinct is not None else ""

    query = f"""
        {{
            colors(filter: {{
                fruitsAggregate: {{
                    count: {{
                        arguments: [name]
                        predicate: {{ eq: {expected_count} }}
                        {distinct_str}
                    }}
                }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert len(result.data["colors"]) == len(expected_color_indices)

    # Get the color IDs from the result
    result_color_ids = {color["id"] for color in result.data["colors"]}

    # Get the expected color IDs
    expected_color_ids = {raw_colors[idx]["id"] for idx in expected_color_indices}

    # Assert that the result contains exactly the expected color IDs
    assert result_color_ids == expected_color_ids

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_aggregate_filtered_and_selected_on_paginated_field(any_async_query: AnyQueryExecutor) -> None:
    """Test that filtering on an aggregate and selecting one leaves the parents uncrossed.

    Regression test for #224: the aggregate join was emitted without correlation to the
    pagination subquery, so every matching parent came back once per matching parent,
    carrying the others' counts.
    """
    query = """
        {
            colorsFilterablePaginated(
                filter: { fruitsAggregate: { sum: { arguments: [sweetness] predicate: { gt: 5 } } } }
                orderBy: { name: ASC }
            ) {
                name
                fruitsAggregate { count }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["colorsFilterablePaginated"] == [
        {"name": "Orange", "fruitsAggregate": {"count": 2}},
        {"name": "Pink", "fruitsAggregate": {"count": 2}},
        {"name": "Red", "fruitsAggregate": {"count": 2}},
        {"name": "Yellow", "fruitsAggregate": {"count": 3}},
    ]


async def test_aggregate_filtered_and_selected_paginates_matching_parents(
    any_async_query: AnyQueryExecutor,
) -> None:
    """Test that limit and offset index the parents the aggregate filter kept."""
    query = """
        {
            colorsFilterablePaginated(
                filter: { fruitsAggregate: { sum: { arguments: [sweetness] predicate: { gt: 5 } } } }
                orderBy: { name: ASC }
                offset: 1
                limit: 2
            ) {
                name
                fruitsAggregate { count }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["colorsFilterablePaginated"] == [
        {"name": "Pink", "fruitsAggregate": {"count": 2}},
        {"name": "Red", "fruitsAggregate": {"count": 2}},
    ]


def _order_by_keys(statement: str) -> list[str]:
    return [key.strip() for key in statement.rsplit("ORDER BY", 1)[1].split(",")]


@pytest.mark.snapshot
async def test_aggregation_filter_through_to_many_relation_also_selected(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a relation reached by an aggregation filter and selected is joined and ordered once."""
    query = """
        {
            colors(filter: { fruits: { farmsAggregate: { count: { predicate: { gt: 1 } } } } }) {
                id
                fruits { id }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["colors"] == [{"id": 1, "fruits": [{"id": 1}, {"id": 2}]}]

    assert query_tracker.query_count == 1
    order_by_keys = _order_by_keys(query_tracker[0].statement_str)
    assert len(order_by_keys) == len(set(order_by_keys))
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_aggregation_filter_through_to_one_relation_also_selected(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    """Test that a to-one relation reached by an aggregation filter and selected is joined and ordered once."""
    query = """
        {
            fruits(filter: { color: { fruitsAggregate: { count: { predicate: { gt: 2 } } } } }) {
                id
                color { id }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert sorted(result.data["fruits"], key=lambda fruit: fruit["id"]) == [
        {"id": 3, "color": {"id": 2}},
        {"id": 4, "color": {"id": 2}},
        {"id": 5, "color": {"id": 2}},
    ]

    assert query_tracker.query_count == 1
    order_by_keys = _order_by_keys(query_tracker[0].statement_str)
    assert len(order_by_keys) == len(set(order_by_keys))
    assert query_tracker[0].statement_formatted == sql_snapshot


async def test_not_or_of_aggregation_and_to_many_through_to_one_relation(
    any_async_query: AnyQueryExecutor, query_tracker: QueryTracker
) -> None:
    """A relation-level ``_not`` of an OR on an aggregation and a to-many keeps the fruits of colors matching neither."""
    query = """
        {
            fruits(
                filter: {
                    color: {
                        _not: {
                            _or: [
                                { fruitsAggregate: { count: { predicate: { gt: 2 } } } }
                                { fruits: { name: { eq: "Apple" } } }
                            ]
                        }
                    }
                }
            ) {
                id
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert sorted(fruit["id"] for fruit in result.data["fruits"]) == [6, 7, 8, 9, 10, 11]
    assert re.search(r"\) IS NOT (TRUE|1) ", query_tracker[0].statement_str, re.IGNORECASE)


async def test_aggregation_filter_through_relation_selected_with_its_own_ordering(
    any_async_query: AnyQueryExecutor,
) -> None:
    """Test that an ordered selection of a relation the filter also reaches keeps its own ordering."""
    query = """
        {
            colors(filter: { fruits: { farmsAggregate: { count: { predicate: { gt: 1 } } } } }) {
                id
                fruits(orderBy: { id: DESC }) { id }
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data

    assert result.data["colors"] == [{"id": 1, "fruits": [{"id": 2}, {"id": 1}]}]

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from tests.integration.fixtures import QueryTracker
    from tests.integration.typing import RawRecordData
    from tests.typing import AnyQueryExecutor


@pytest.mark.snapshot
async def test_custom_apply_filter_sweeter_than(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    threshold = 5
    expected_ids = {fruit["id"] for fruit in raw_fruits if fruit["sweetness"] >= threshold}
    query = f"""
        {{
            fruitsFineGrained(filter: {{ sweeterThan: {threshold} }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["fruitsFineGrained"]} == expected_ids
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_custom_apply_filter_in_strategy(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    threshold = 5
    expected_ids = {fruit["id"] for fruit in raw_fruits if fruit["sweetness"] >= threshold}
    query = f"""
        {{
            fruitsFineGrained(filter: {{ sweeterThanIn: {threshold} }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["fruitsFineGrained"]} == expected_ids
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("field", [pytest.param("sweeterThan", id="exists"), pytest.param("sweeterThanIn", id="in")])
async def test_null_custom_apply_filter_is_ignored(
    field: str, any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    """Test that a custom apply filter set to null filters nothing."""
    result = await maybe_async(any_async_query(f"{{ fruitsFineGrained(filter: {{ {field}: null }}) {{ id }} }}"))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["fruitsFineGrained"]} == {fruit["id"] for fruit in raw_fruits}


async def test_custom_apply_filter_under_or(any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData) -> None:
    threshold = 8
    target_name = raw_fruits[0]["name"]
    expected_ids = {
        fruit["id"] for fruit in raw_fruits if fruit["sweetness"] >= threshold or fruit["name"] == target_name
    }
    query = f"""
        {{
            fruitsFineGrained(
                filter: {{
                    _or: [
                        {{ sweeterThan: {threshold} }}
                        {{ name: {{ eq: "{target_name}" }} }}
                    ]
                }}
            ) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["fruitsFineGrained"]} == expected_ids


@pytest.mark.snapshot
async def test_declared_aggregate_count_filter(
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """A declared aggregate filter still filters by count against the database."""
    threshold = 1
    counts: dict[Any, int] = {}
    for fruit in raw_fruits:
        counts[fruit["color_id"]] = counts.get(fruit["color_id"], 0) + 1
    expected_ids = {color["id"] for color in raw_colors if counts.get(color["id"], 0) > threshold}

    query = f"""
        {{
            colorsFineGrained(filter: {{
                fruitsAggregate: {{ count: {{ arguments: [id], predicate: {{ gt: {threshold} }} }} }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["colorsFineGrained"]} == expected_ids
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_declared_aggregate_sum_filter(
    any_async_query: AnyQueryExecutor,
    raw_colors: RawRecordData,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """A declared aggregate filter narrowed to sum(sweetness) filters against the database.

    `count` narrowing goes through the same code path as an unmarked column filter; `sum`
    narrowing takes a different path (`arguments_type`/`EnumBackend`) that was previously only
    exercised by rejection tests, never against real data.
    """
    threshold = 10
    sums: dict[Any, int] = {}
    for fruit in raw_fruits:
        sums[fruit["color_id"]] = sums.get(fruit["color_id"], 0) + fruit["sweetness"]
    expected_ids = {color["id"] for color in raw_colors if sums.get(color["id"], 0) >= threshold}
    # Both a matching and a non-matching color must exist, or the query and a broken filter
    # (e.g. one that ignores the predicate) would return the same rows.
    assert expected_ids
    assert expected_ids != {color["id"] for color in raw_colors}

    query = f"""
        {{
            colorsFineGrained(filter: {{
                fruitsAggregate: {{ sum: {{ arguments: [sweetness], predicate: {{ gte: {threshold} }} }} }}
            }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data is not None
    assert {row["id"] for row in result.data["colorsFineGrained"]} == expected_ids
    assert query_tracker[0].statement_formatted == sql_snapshot

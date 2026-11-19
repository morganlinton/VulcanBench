from __future__ import annotations

from datetime import timedelta
from typing import TYPE_CHECKING, Any

import msgspec
import pytest
from sqlalchemy import Insert, MetaData, insert

from tests.integration.fixtures import QueryTracker
from tests.integration.models import IntervalModel, interval_metadata
from tests.integration.types import mysql as mysql_types
from tests.integration.types import postgres as postgres_types
from tests.integration.types import sqlite as sqlite_types
from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.typing import SupportedDialect


seconds_in_year = 60 * 60 * 24 * 365.25
seconds_in_month = seconds_in_year / 12
seconds_in_day = 60 * 60 * 24


@pytest.fixture
def metadata() -> MetaData:
    return interval_metadata


@pytest.fixture
def seed_insert_statements(raw_intervals: RawRecordData) -> list[Insert]:
    return [insert(IntervalModel).values(raw_intervals)]


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.IntervalAsyncQuery
    if dialect == "mysql":
        return mysql_types.IntervalAsyncQuery
    if dialect == "sqlite":
        return sqlite_types.IntervalAsyncQuery
    pytest.skip(f"Interval tests can't be run on this dialect: {dialect}")


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.IntervalSyncQuery
    if dialect == "mysql":
        return mysql_types.IntervalSyncQuery
    if dialect == "sqlite":
        return sqlite_types.IntervalSyncQuery
    pytest.skip(f"Interval tests can't be run on this dialect: {dialect}")


@pytest.mark.parametrize(
    ("component", "value", "expected_ids"),
    [
        pytest.param("days", timedelta(weeks=1, days=3, hours=12).total_seconds() / seconds_in_day, [1], id="days"),
        pytest.param("hours", timedelta(weeks=1, days=3, hours=12).total_seconds() / 3600, [1], id="hours"),
        pytest.param("minutes", timedelta(weeks=1, days=3, hours=12).total_seconds() / 60, [1], id="minutes"),
        pytest.param("seconds", timedelta(weeks=1, days=3, hours=12).total_seconds(), [1], id="totalSeconds"),
    ],
)
@pytest.mark.snapshot
async def test_timedelta_components(
    component: str,
    value: int,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_intervals: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            intervals(filter: {{ timeDeltaCol: {{ {component}: {{ eq: {value} }} }} }}) {{
                id
                timeDeltaCol
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["intervals"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["intervals"][i]["id"] == raw_intervals[expected_id]["id"]

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_timedelta_output(
    any_async_query: AnyQueryExecutor,
    raw_intervals: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = """
        {
            intervals {
                id
                timeDeltaCol
            }
        }
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["intervals"]) == len(raw_intervals)

    for interval in result.data["intervals"]:
        expected_interval = next(f for f in raw_intervals if f["id"] == interval["id"])
        assert interval["timeDeltaCol"] == msgspec.json.encode(expected_interval["time_delta_col"]).decode()

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    "dto_filter",
    [
        pytest.param("{ timeDeltaCol: { days: null } }", id="part"),
        pytest.param("{ timeDeltaCol: { hours: { eq: null } } }", id="part-operator"),
        pytest.param("{ _not: { timeDeltaCol: { seconds: { gt: null }, lt: null } } }", id="not-part-operator"),
    ],
)
async def test_null_part_operator_is_ignored(
    dto_filter: str, any_async_query: AnyQueryExecutor, raw_intervals: RawRecordData
) -> None:
    """Test that an interval part, or one of its operators, set to null is ignored."""
    result = await maybe_async(any_async_query(f"{{ intervals(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data
    assert sorted(row["id"] for row in result.data["intervals"]) == sorted(row["id"] for row in raw_intervals)

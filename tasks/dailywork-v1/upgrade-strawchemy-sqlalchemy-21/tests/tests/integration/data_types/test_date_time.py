from __future__ import annotations

from datetime import date, datetime, time, timedelta
from typing import TYPE_CHECKING, Any

import pytest
from sqlalchemy import Insert, MetaData, insert

from tests.integration.fixtures import QueryTracker
from tests.integration.models import DateTimeModel, date_time_metadata
from tests.integration.types import mysql as mysql_types
from tests.integration.types import postgres as postgres_types
from tests.integration.types import sqlite as sqlite_types
from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy.typing import SupportedDialect

pytestmark = [pytest.mark.integration]


def _iso_boundary_records() -> RawRecordData:
    start, end = date(2019, 12, 20), date(2021, 1, 10)
    days = [start + timedelta(days=offset) for offset in range((end - start).days + 1)]
    return [
        {
            "id": index + 1,
            "date_col": day,
            "time_col": time(12, 0),
            "datetime_col": datetime.combine(day, time(12, 0)),
        }
        for index, day in enumerate(days)
    ]


@pytest.fixture
def metadata() -> MetaData:
    return date_time_metadata


@pytest.fixture
def seed_insert_statements(raw_date_times: RawRecordData) -> list[Insert]:
    return [insert(DateTimeModel).values(raw_date_times)]


@pytest.fixture
def async_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.DateTimeAsyncQuery
    if dialect == "mysql":
        return mysql_types.DateTimeAsyncQuery
    if dialect == "sqlite":
        return sqlite_types.DateTimeAsyncQuery
    pytest.skip(f"Date/Time tests can't be run on this dialect: {dialect}")


@pytest.fixture
def sync_query(dialect: SupportedDialect) -> type[Any]:
    if dialect == "postgresql":
        return postgres_types.DateTimeSyncQuery
    if dialect == "mysql":
        return mysql_types.DateTimeSyncQuery
    if dialect == "sqlite":
        return sqlite_types.DateTimeSyncQuery
    pytest.skip(f"Date/Time tests can't be run on this dialect: {dialect}")


# Tests for date/time component filters
@pytest.mark.parametrize(
    ("component", "value", "expected_ids"),
    [
        pytest.param("year", 2023, [0], id="year"),
        pytest.param("month", 1, [0], id="month"),
        pytest.param("day", 15, [0], id="day"),
        pytest.param("weekDay", 6, [1], id="weekDay"),  # Sunday is 6
        pytest.param("week", 2, [0], id="week"),  # Second week of the year
        pytest.param("quarter", 1, [0, 2], id="quarter"),  # First quarter
        pytest.param("isoYear", 2023, [0], id="isoYear"),
        pytest.param("isoWeekDay", 7, [0], id="isoWeekDay"),  # Sunday is 7 in ISO
    ],
)
@pytest.mark.snapshot
async def test_date_components(
    component: str,
    value: int,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_date_times: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            dateTimes(filter: {{ dateCol: {{ {component}: {{ eq: {value} }} }} }}) {{
                id
                dateCol
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["dateTimes"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["dateTimes"][i]["id"] == raw_date_times[expected_id]["id"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("component", "value", "expected_ids"),
    [
        pytest.param("hour", 14, [0], id="hour"),
        pytest.param("minute", 30, [0], id="minute"),
        pytest.param("second", 45, [0], id="second"),
    ],
)
@pytest.mark.snapshot
async def test_time_components(
    component: str,
    value: int,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_date_times: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            dateTimes(filter: {{ timeCol: {{ {component}: {{ eq: {value} }} }} }}) {{
                id
                timeCol
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["dateTimes"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["dateTimes"][i]["id"] == raw_date_times[expected_id]["id"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize(
    ("component", "value", "expected_ids"),
    [
        pytest.param("hour", 14, [0], id="hour"),
        pytest.param("minute", 30, [0], id="minute"),
        pytest.param("second", 45, [0], id="second"),
        pytest.param("year", 2023, [0], id="year"),
        pytest.param("month", 1, [0], id="month"),
        pytest.param("day", 15, [0], id="day"),
        pytest.param("weekDay", 6, [1], id="weekDay"),  # Sunday is 0, saturday is 6
        pytest.param("week", 2, [0], id="week"),
        pytest.param("quarter", 1, [0, 2], id="quarter"),
        pytest.param("isoYear", 2023, [0], id="isoYear"),
        pytest.param("isoWeekDay", 7, [0], id="isoWeekDay"),
    ],
)
@pytest.mark.snapshot
async def test_datetime_components(
    component: str,
    value: int,
    expected_ids: list[int],
    any_async_query: AnyQueryExecutor,
    raw_date_times: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    query = f"""
        {{
            dateTimes(filter: {{ datetimeCol: {{ {component}: {{ eq: {value} }} }} }}) {{
                id
                datetimeCol
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    assert not result.errors
    assert result.data
    assert len(result.data["dateTimes"]) == len(expected_ids)
    for i, expected_id in enumerate(expected_ids):
        assert result.data["dateTimes"][i]["id"] == raw_date_times[expected_id]["id"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("raw_date_times", [_iso_boundary_records()], ids=["iso-boundaries"])
@pytest.mark.parametrize(
    ("component", "attribute", "value"),
    [
        pytest.param("isoYear", "year", 2020, id="isoYear"),
        pytest.param("week", "week", 1, id="week"),
        pytest.param("week", "week", 53, id="week-53"),
        pytest.param("isoWeekDay", "weekday", 7, id="isoWeekDay"),
    ],
)
async def test_iso_components_match_isocalendar(
    component: str, attribute: str, value: int, any_async_query: AnyQueryExecutor, raw_date_times: RawRecordData
) -> None:
    """Test that an ISO component filter selects exactly the rows isocalendar() agrees with."""
    query = f"""
        {{
            dateTimes(filter: {{ dateCol: {{ {component}: {{ eq: {value} }} }} }}) {{
                id
            }}
        }}
    """
    result = await maybe_async(any_async_query(query))
    expected = [row["id"] for row in raw_date_times if getattr(row["date_col"].isocalendar(), attribute) == value]

    assert not result.errors
    assert result.data
    assert sorted(row["id"] for row in result.data["dateTimes"]) == expected


@pytest.mark.parametrize(
    "dto_filter",
    [
        pytest.param("{ dateCol: { year: null } }", id="date-part"),
        pytest.param("{ dateCol: { year: { eq: null } } }", id="date-part-operator"),
        pytest.param("{ _not: { dateCol: { month: { gt: null }, lte: null } } }", id="not-date-part-operator"),
        pytest.param("{ timeCol: { hour: null } }", id="time-part"),
        pytest.param("{ timeCol: { hour: { lt: null } } }", id="time-part-operator"),
        pytest.param("{ datetimeCol: { gte: null, isoYear: { eq: null }, second: null } }", id="datetime-parts"),
        pytest.param("{ _not: { datetimeCol: { minute: { eq: null } } } }", id="not-datetime-part-operator"),
    ],
)
async def test_null_part_operator_is_ignored(
    dto_filter: str, any_async_query: AnyQueryExecutor, raw_date_times: RawRecordData
) -> None:
    """Test that a date or time part, or one of its operators, set to null is ignored."""
    result = await maybe_async(any_async_query(f"{{ dateTimes(filter: {dto_filter}) {{ id }} }}"))
    assert not result.errors
    assert result.data
    assert sorted(row["id"] for row in result.data["dateTimes"]) == sorted(row["id"] for row in raw_date_times)

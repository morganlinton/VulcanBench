from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tests.integration.fixtures import QueryTracker
from tests.integration.typing import RawRecordData
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from strawchemy import StrawchemyConfig

pytestmark = [pytest.mark.integration]


async def test_default_order_by_applied_when_no_client_order(
    any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData
) -> None:
    """Default ordering by name ASC is applied when the client sends no orderBy.

    The default-ordered result is compared against an explicit ``orderBy: { name: ASC }``
    query issued to the same database, so the assertion is independent of the database's
    text collation (which differs between sqlite's binary ordering and Postgres/MySQL
    locale-aware ordering).
    """
    default = await maybe_async(any_async_query("{ fruitsDefaultOrderNoPagination { id name } }"))
    explicit = await maybe_async(
        any_async_query("{ fruitsDefaultOrderNoPagination(orderBy: { name: ASC }) { id name } }")
    )
    assert not default.errors
    assert not explicit.errors
    assert default.data
    assert explicit.data
    assert len(default.data["fruitsDefaultOrderNoPagination"]) == len(raw_fruits)
    # No client order falls back to default_order_by (name ASC), matching explicit name ASC.
    assert default.data["fruitsDefaultOrderNoPagination"] == explicit.data["fruitsDefaultOrderNoPagination"]


async def test_client_order_overrides_default(any_async_query: AnyQueryExecutor, raw_fruits: RawRecordData) -> None:
    """A client orderBy fully overrides default_order_by.

    Compares the descending result against the reverse of the ascending result (names are
    unique in the fixture), keeping the assertion independent of database text collation.
    """
    asc = await maybe_async(any_async_query("{ fruitsDefaultOrderNoPagination(orderBy: { name: ASC }) { id name } }"))
    desc = await maybe_async(any_async_query("{ fruitsDefaultOrderNoPagination(orderBy: { name: DESC }) { id name } }"))
    assert not asc.errors
    assert not desc.errors
    assert asc.data
    assert desc.data
    assert len(desc.data["fruitsDefaultOrderNoPagination"]) == len(raw_fruits)
    asc_names = [row["name"] for row in asc.data["fruitsDefaultOrderNoPagination"]]
    desc_names = [row["name"] for row in desc.data["fruitsDefaultOrderNoPagination"]]
    # Client DESC ordering is honored over the default ASC, producing the reverse order.
    assert desc_names == list(reversed(asc_names))


@pytest.mark.snapshot
async def test_default_order_with_pk_tiebreaker(
    any_async_query: AnyQueryExecutor,
    config: StrawchemyConfig,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """With deterministic_ordering=True the PK is appended after the default column."""
    # True is the StrawchemyConfig default
    config.deterministic_ordering = True
    result = await maybe_async(any_async_query("{ fruitsDefaultOrderNoPagination { id name } }"))
    assert not result.errors
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_without_pk_tiebreaker(
    any_async_query: AnyQueryExecutor,
    config: StrawchemyConfig,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """With deterministic_ordering=False only the default column is emitted (no PK)."""
    config.deterministic_ordering = False
    result = await maybe_async(any_async_query("{ fruitsDefaultOrderNoPagination { id name } }"))
    assert not result.errors
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_under_pagination(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Default ordering is adapted to the subquery alias when paginating (limit/offset)."""
    result = await maybe_async(any_async_query("{ fruitsDefaultOrder(limit: 2, offset: 1) { id name } }"))
    assert not result.errors
    assert result.data
    assert len(result.data["fruitsDefaultOrder"]) == 2
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_multi_column(
    any_async_query: AnyQueryExecutor,
    raw_fruits: RawRecordData,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """Multi-column default_order_by list is applied: sweetness DESC, name ASC, then PK tiebreaker."""
    result = await maybe_async(any_async_query("{ fruitsDefaultOrderMulti { id name sweetness } }"))
    assert not result.errors
    assert result.data
    assert len(result.data["fruitsDefaultOrderMulti"]) == len(raw_fruits)
    expected_names = [row["name"] for row in sorted(raw_fruits, key=lambda r: (-r["sweetness"], r["name"]))]
    assert [row["name"] for row in result.data["fruitsDefaultOrderMulti"]] == expected_names
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_with_distinct_on(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """distinct_on combined with default_order_by uses the default ordering inside the RANK window."""
    result = await maybe_async(any_async_query("{ fruitsDefaultOrderDistinct(distinctOn: [name]) { id name } }"))
    assert not result.errors
    assert result.data
    assert len(result.data["fruitsDefaultOrderDistinct"]) > 0
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_not_applied_to_nested_same_named_column(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """A root default ordering on ``name`` leaves nested rows ordered by their primary key, not their own ``name``."""
    result = await maybe_async(any_async_query("{ colorsDefaultOrder { name fruits(limit: 1) { id name } } }"))
    assert not result.errors
    assert result.data
    red = next(color for color in result.data["colorsDefaultOrder"] if color["name"] == "Red")
    assert [fruit["name"] for fruit in red["fruits"]] == ["Apple"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.snapshot
async def test_default_order_not_applied_to_nested_missing_column(
    any_async_query: AnyQueryExecutor,
    query_tracker: QueryTracker,
    sql_snapshot: SnapshotAssertion,
) -> None:
    """A root default ordering on a column the nested model lacks does not fail the nested plan."""
    result = await maybe_async(any_async_query("{ fruitsDefaultOrderSweetness { name farms(limit: 10) { name } } }"))
    assert not result.errors
    assert result.data
    apple = next(fruit for fruit in result.data["fruitsDefaultOrderSweetness"] if fruit["name"] == "Apple")
    assert [farm["name"] for farm in apple["farms"]] == ["Apple farm", "Apple farm 2", "Apple farm 3"]
    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot

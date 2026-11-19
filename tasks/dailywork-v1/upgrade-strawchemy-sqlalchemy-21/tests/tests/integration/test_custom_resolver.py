from __future__ import annotations

from typing import TYPE_CHECKING, Literal

import pytest

from tests.integration.utils import to_graphql_representation
from tests.typing import AnyQueryExecutor
from tests.utils import maybe_async

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

    from tests.integration.fixtures import QueryTracker


pytestmark = [pytest.mark.integration]


@pytest.mark.snapshot
async def test_get_one(
    any_query: AnyQueryExecutor, query_tracker: QueryTracker, sql_snapshot: SnapshotAssertion
) -> None:
    result = await maybe_async(any_query("{ redColor { name } }"))

    assert not result.errors
    assert result.data
    assert result.data["redColor"] == {"name": "Red"}

    assert query_tracker.query_count == 1
    assert query_tracker[0].statement_formatted == sql_snapshot


@pytest.mark.parametrize("color", ["unknown", "Pink"])
@pytest.mark.snapshot
async def test_get_one_or_none(color: Literal["unknown", "Pink"], any_query: AnyQueryExecutor) -> None:
    result = await maybe_async(
        any_query(f"""
            {{
                  getColor(color: {to_graphql_representation(color, "input")}) {{
                    name
                }}
            }}
        """)
    )

    assert not result.errors
    assert result.data
    assert result.data["getColor"] == ({"name": "Pink"} if color == "Pink" else None)

from __future__ import annotations

from typing import Any
from unittest.mock import MagicMock

import pytest

from strawchemy.dto.strawberry import AggregationFilter, Filter
from strawchemy.schema.filters import DateComparison, EqualityComparison, GraphQLComparison, OrderComparison


def _agg(tag: str) -> AggregationFilter:
    """Builds a stand-in AggregationFilter identifiable by ``tag``."""
    agg = MagicMock(spec=AggregationFilter)
    agg.tag = tag
    return agg


def test_iter_aggregation_filters_walks_and_or_not_branches() -> None:
    """iter_aggregation_filters yields every AggregationFilter under and_/or_/not_ in traversal order."""
    inner = Filter(and_=[_agg("a")])
    outer = Filter(and_=[inner, _agg("b")], or_=[Filter(and_=[_agg("c")])], not_=Filter(and_=[_agg("d")]))
    assert [aggregation_filter.tag for aggregation_filter in outer.iter_aggregation_filters()] == ["a", "b", "c", "d"]  # ty:ignore[unresolved-attribute]


@pytest.mark.parametrize(
    ("comparison", "expected"),
    [
        pytest.param(DateComparison(), False, id="no-operator"),
        pytest.param(DateComparison(year=OrderComparison[Any]()), False, id="empty-nested"),  # ty:ignore[unknown-argument]
        pytest.param(DateComparison(year=OrderComparison[Any](gt=2)), True, id="nested"),  # ty:ignore[unknown-argument]
        pytest.param(DateComparison(eq=None), False, id="null"),  # ty:ignore[unknown-argument]
        pytest.param(DateComparison(year=None), False, id="null-nested"),  # ty:ignore[unknown-argument]
        pytest.param(DateComparison(year=OrderComparison[Any](eq=None)), False, id="nested-null"),  # ty:ignore[unknown-argument]
        pytest.param(EqualityComparison[Any](in_=[]), True, id="empty-list"),  # ty:ignore[unknown-argument]
    ],
)
def test_has_operator(comparison: GraphQLComparison, expected: bool) -> None:
    assert comparison.has_operator() is expected

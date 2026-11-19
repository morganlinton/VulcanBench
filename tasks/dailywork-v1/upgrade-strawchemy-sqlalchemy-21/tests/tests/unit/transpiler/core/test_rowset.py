"""Tests for ``RowSet`` and ``Projection``."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import pytest
from sqlalchemy import func, true
from sqlalchemy.orm import aliased

from strawchemy.transpiler._core.plan import QueryPlan
from strawchemy.transpiler._core.rowset import Join, OrderPriority, Projection, RowSet
from strawchemy.typing import QueryNodeType
from tests.unit.models import Color, Fruit

if TYPE_CHECKING:
    from sqlalchemy import Select


def _node() -> QueryNodeType:
    return cast("QueryNodeType", object())


def _join(key_node: QueryNodeType) -> Join:
    return Join(key=("relation", key_node), target=aliased(Color.__mapper__), onclause=None, is_outer=True, alias=None)


def test_with_join_keeps_first_join_for_key() -> None:
    """Two joins share a key: the first one added stays."""
    node = _node()
    first, second = _join(node), _join(node)
    rows = RowSet.over(aliased(Color.__mapper__))

    assert rows.with_join(first).with_join(second).join(("relation", node)) is first


def test_only_filters_true_for_where_and_order() -> None:
    """A RowSet with WHERE and ORDER BY only is filter-only."""
    alias = aliased(Color.__mapper__)
    rows = RowSet.over(alias).with_where(alias.name == "red").with_order_by(OrderPriority.CLIENT, alias.name.asc())

    assert rows.only_filters()


@pytest.mark.parametrize("option", [{"limit": 1}, {"offset": 1}, {"distinct_on": (Color.name,)}])
def test_only_filters_false_with_limit_offset_or_distinct(option: dict[str, Any]) -> None:
    """A RowSet with limit, offset or DISTINCT ON is not filter-only."""
    rows = RowSet(source=aliased(Color.__mapper__), **option)

    assert not rows.only_filters()


def test_only_filters_false_when_edit_adds_order_by() -> None:
    """An edit adding ORDER BY is not filter-only, one adding WHERE only is."""
    rows = RowSet.over(aliased(Color.__mapper__))

    def order(statement: Select[Any]) -> Select[Any]:
        return statement.order_by(Color.name)

    def filt(statement: Select[Any]) -> Select[Any]:
        return statement.where(Color.name == "red")

    assert not rows.with_edit(order).only_filters()
    assert rows.with_edit(filt).only_filters()


def test_only_filters_unordered_ignores_edit_order_by() -> None:
    """With ``unordered``, an edit adding ORDER BY and WHERE is filter-only, one adding a join is not."""
    alias = aliased(Color.__mapper__)
    rows = RowSet.over(alias).with_edit(lambda statement: statement.where(alias.name == "red").order_by(alias.name))

    assert rows.only_filters(unordered=True)
    assert not rows.with_edit(lambda statement: statement.join(Fruit, true())).only_filters(unordered=True)


def test_edits_where_returns_the_where_edits_add() -> None:
    """``edits_where`` is the WHERE the edits add to the source, ``None`` for edits adding only ORDER BY."""
    alias = aliased(Color.__mapper__)
    rows = RowSet.over(alias)
    ordered = rows.with_edit(lambda statement: statement.order_by(alias.name))
    filtered = rows.with_edit(lambda statement: statement.where(alias.name == "red"))

    assert ordered.edits_where() is None
    where = filtered.edits_where()
    assert where is not None
    assert where.compare(alias.name == "red")


def test_projection_merge_keeps_both_sides() -> None:
    """Merging keeps both entity maps and concatenates columns in order."""
    left_node, right_node = _node(), _node()
    left_alias, right_alias = aliased(Color.__mapper__), aliased(Color.__mapper__)
    left = Projection.over(left_node, left_alias).with_columns(left_alias.name)
    right = Projection.over(right_node, right_alias).with_columns(right_alias.name)

    merged = left.merge(right)

    assert dict(merged.entities) == {left_node: left_alias, right_node: right_alias}
    assert merged.columns == (left_alias.name, right_alias.name)


def test_only_filters_false_when_edit_adds_join() -> None:
    """An edit adding a join makes the RowSet not filter-only, without raising."""
    rows = RowSet.over(aliased(Color.__mapper__)).with_edit(lambda statement: statement.join(Fruit, true()))

    assert not rows.only_filters()


def test_with_root_aggregation_maps_label_outside_columns() -> None:
    """A root aggregation is in column_map and root_aggregation_functions, not in columns."""
    node, alias = _node(), aliased(Color.__mapper__)
    label = func.count(alias.id).label("count")
    projection = Projection.over(_node(), alias).with_root_aggregation(node, label)
    plan = QueryPlan(rows=RowSet.over(alias), projection=projection, context=cast("Any", None))

    assert plan.column_map[node] is label
    assert label in plan.root_aggregation_functions
    assert label not in projection.columns

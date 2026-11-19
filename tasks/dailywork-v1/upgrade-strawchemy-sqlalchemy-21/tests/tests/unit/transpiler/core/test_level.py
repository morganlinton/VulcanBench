"""Tests for the ``Level`` services: direct and path columns, and aggregate joins."""

from __future__ import annotations

import typing
from dataclasses import replace
from functools import partial
from typing import Any, cast

from inline_snapshot import snapshot
from sqlalchemy import inspect
from sqlalchemy.dialects import postgresql, sqlite
from sqlalchemy.orm import aliased

from strawchemy import Strawchemy
from strawchemy.dto.strawberry import QueryNode
from strawchemy.schema.filters.inputs import TextComparison
from strawchemy.transpiler._core.level import Level, PlanContext, _same_rows
from strawchemy.transpiler._core.pipeline import Pipeline, Pipelines
from strawchemy.transpiler._core.render import clause_element
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._core.rowset import AggregateJoin, Join, OrderPriority, Projection, RowSet
from strawchemy.transpiler.hook import QueryHook
from strawchemy.typing import QueryNodeType
from tests.unit.models import Color, Fruit, SQLDataTypes, User
from tests.utils import as_dto

if typing.TYPE_CHECKING:
    from sqlalchemy.engine import Dialect
    from sqlalchemy.orm import DeclarativeBase, QueryableAttribute
    from sqlalchemy.sql import ColumnElement

    from strawchemy.dto.strawberry import BooleanFilterDTO

_strawchemy = Strawchemy("postgresql")


@_strawchemy.type(User, include="all")
class _UserType: ...


@_strawchemy.filter(User, include="all")
class _UserFilter: ...


@_strawchemy.type(Color, include="all")
class _ColorType: ...


@_strawchemy.type(SQLDataTypes, include="all")
class _SQLDataTypesType: ...


def _unwrap(type_: object) -> type[Any]:
    """Returns the non-None member of an optional annotation."""
    args = [arg for arg in typing.get_args(type_) if arg is not type(None)]
    return cast("type[Any]", args[0] if args else type_)


_GROUP_FILTER = _unwrap(_UserFilter.__annotations__["group"])
_EMPTY = Pipeline(())
_PIPELINES = Pipelines(root=_EMPTY, relation=_EMPTY, exists=_EMPTY, dml=_EMPTY)


def _level(
    model: type[DeclarativeBase],
    selection: QueryNodeType,
    dialect: Dialect | None = None,
    dto_filter: BooleanFilterDTO | None = None,
) -> Level:
    request = QueryRequest(
        model=model,
        selection_tree=selection,
        dto_filter=dto_filter,
        order_by=(),
        distinct_on=(),
        limit=None,
        offset=None,
        allow_null=False,
    )
    context = PlanContext.create(model, dialect or postgresql.psycopg2.dialect(), pipelines=_PIPELINES)
    return Level.root(request, context)


def _same(left: ColumnElement[Any], right: QueryableAttribute[Any]) -> bool:
    """Tells whether a column is the SQL expression of an ORM attribute."""
    return left.compare(clause_element(right))


def _child(node: QueryNodeType, name: str) -> QueryNodeType:
    return node.insert_child(node.value.type_.__dto_field_definitions__[name])


def _user_group_color() -> tuple[QueryNodeType, QueryNodeType, QueryNodeType, QueryNodeType, QueryNodeType]:
    """Returns the tree ``user { group { color { name id } } }`` as root, group, color, name, id."""
    root = QueryNode.root_node(User)
    group = root.insert_child(as_dto(_UserType).__dto_field_definitions__["group"])
    color = _child(group, "color")
    return root, group, color, _child(color, "name"), _child(color, "id")


def _color_aggregates() -> tuple[QueryNodeType, QueryNodeType]:
    """Returns ``color { fruitsAggregate { count sum { sweetness } } }`` as root and the aggregation node."""
    root = QueryNode.root_node(Color)
    aggregation = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits_aggregate"])
    _child(aggregation, "count")
    _child(_child(aggregation, "sum"), "sweetness")
    return root, aggregation


def test_root_alias_is_named_after_the_table() -> None:
    """The root level selects from a flat alias named after the model's table."""
    root, *_ = _user_group_color()

    level = _level(User, root)

    assert inspect(level.alias).name == "user"
    assert level.node is root
    assert level.kind == "root"


def test_dml_alias_targets_the_table_columns() -> None:
    """The DML level's alias reads the table's own columns."""
    root, *_ = _user_group_color()
    level = _level(User, root)

    dml = Level.dml(level.request, level.context)

    assert inspect(dml.alias).selectable is User.__table__
    assert dml.kind == "dml"


def test_column_reads_from_the_level_alias() -> None:
    """A direct column of the level's model is read from the level alias."""
    root = QueryNode.root_node(User)
    name = root.insert_child(as_dto(_UserType).__dto_field_definitions__["name"])
    level = _level(User, root)

    column = level.column(name)

    assert _same(column, level.alias.name)


def test_column_extracts_the_json_path() -> None:
    """A node with a JSON path is read through ``jsonb_path_query_first``, defaulting to an empty object."""
    root = QueryNode.root_node(SQLDataTypes)
    dict_col = root.insert_child(as_dto(_SQLDataTypesType).__dto_field_definitions__["dict_col"])
    dict_col.metadata.data.json_path = "$.key"
    level = _level(SQLDataTypes, root)

    assert str(level.column(dict_col).compile(dialect=postgresql.psycopg2.dialect())) == snapshot(
        "coalesce(jsonb_path_query_first(sql_data_types.dict_col, CAST(%(param_1)s AS JSONPATH)), CAST(%(param_2)s::JSONB AS JSONB))"
    )


def test_path_column_joins_each_hop_once() -> None:
    """Reaching ``group.color.name`` then ``group.color.id`` adds one join per hop, the second call none."""
    root, group, color, name, id_ = _user_group_color()
    level = _level(User, root)
    rows = RowSet.over(level.alias)

    name_column, rows = level.path_column(name, rows)
    id_column, second_rows = level.path_column(id_, rows)

    assert second_rows is rows
    assert set(rows.joins) == {("relation", group), ("relation", color)}
    color_join = rows.joins["relation", color]
    assert color_join.alias is not None
    assert _same(name_column, color_join.alias.name)
    assert _same(id_column, color_join.alias.id)
    assert all(join.is_outer for join in rows.joins.values())


def test_path_column_inner_joins_the_filter_join_path() -> None:
    """A hop the filter goes through is inner-joined; the rest stays outer."""
    root, group, color, name, _ = _user_group_color()
    level = _level(User, root, dto_filter=_UserFilter(group=_GROUP_FILTER(name=TextComparison(eq="x"))))  # ty: ignore[unknown-argument]  # input fields are generated at runtime

    _, rows = level.path_column(name, RowSet.over(level.alias))

    assert not rows.joins["relation", group].is_outer
    assert rows.joins["relation", color].is_outer


def test_path_column_of_a_direct_column_adds_no_join() -> None:
    """A column of the level's own model is returned with the RowSet unchanged."""
    root = QueryNode.root_node(User)
    name = root.insert_child(as_dto(_UserType).__dto_field_definitions__["name"])
    level = _level(User, root)
    rows = RowSet.over(level.alias)

    column, same_rows = level.path_column(name, rows)

    assert same_rows is rows
    assert _same(column, level.alias.name)


def test_aggregate_join_holds_every_requested_function() -> None:
    """``aggregate(count)`` then ``aggregate(sum)`` share one ``AggregateJoin`` built with both functions."""
    root, aggregation = _color_aggregates()
    level = _level(Color, root)
    functions = level.request.aggregate_functions(aggregation)
    count, sum_ = sorted(functions.values(), key=lambda function: function.name)

    count_column, rows = level.aggregate(count, RowSet.over(level.alias))
    sum_column, second_rows = level.aggregate(sum_, rows)

    assert second_rows is rows
    join = rows.joins["aggregate", aggregation]
    assert isinstance(join, AggregateJoin)
    assert set(join.columns) == {count.node, sum_.node}
    assert count_column is join.columns[count.node]
    assert sum_column is join.columns[sum_.node]
    assert not join.is_outer


def test_aggregate_join_is_outer_without_lateral() -> None:
    """On a database without LATERAL the aggregate join is an outer CTE join."""
    root, aggregation = _color_aggregates()
    level = _level(Color, root, sqlite.dialect())
    count = next(function for function in level.request.aggregate_functions(aggregation).values())

    _, rows = level.aggregate(count, RowSet.over(level.alias))

    assert rows.joins["aggregate", aggregation].is_outer


def test_projected_aggregate_reuses_rows_join() -> None:
    """When the RowSet has the aggregate join, the projection gets its column and no new join."""
    root, aggregation = _color_aggregates()
    level = _level(Color, root)
    count, sum_ = sorted(level.request.aggregate_functions(aggregation).values(), key=lambda function: function.name)
    _, rows = level.aggregate(count, RowSet.over(level.alias))
    projection = Projection.over(root, level.alias)

    column, same_projection = level.projected_aggregate(sum_, rows, projection)

    assert same_projection is projection
    assert column is rows.joins["aggregate", aggregation].columns[sum_.node]  # ty: ignore[unresolved-attribute]


def test_projected_aggregate_joins_on_the_projection() -> None:
    """Without a row-stage aggregate join, the join is added to the projection and the RowSet is untouched."""
    root, aggregation = _color_aggregates()
    level = _level(Color, root)
    count = next(function for function in level.request.aggregate_functions(aggregation).values())
    rows = RowSet.over(level.alias)

    column, projection = level.projected_aggregate(count, rows, Projection.over(root, level.alias))

    join = projection.joins["aggregate", aggregation]
    assert isinstance(join, AggregateJoin)
    assert column is join.columns[count.node]
    assert not rows.joins


def test_same_rows_ignores_order_and_page() -> None:
    """Rows differing in ORDER BY, limit and offset only, with partials of one hook on one alias, are the same rows."""
    fruit = aliased(Fruit.__mapper__, flat=True)
    hook = QueryHook[Fruit]()
    first = RowSet.over(fruit).with_edit(partial(hook.apply_hook, alias=fruit))
    second = replace(
        RowSet.over(fruit).with_edit(partial(hook.apply_hook, alias=fruit)), limit=2, offset=1
    ).with_order_by(OrderPriority.CLIENT, fruit.name.asc())

    assert _same_rows(first, first)
    assert _same_rows(first, second)


def test_same_rows_tells_other_edits_and_joins_apart() -> None:
    """Rows whose hook runs on another alias, or with a join of their own, are other rows."""
    fruit, other = aliased(Fruit.__mapper__, flat=True), aliased(Fruit.__mapper__, flat=True)
    hook = QueryHook[Fruit]()
    rows = RowSet.over(fruit).with_edit(partial(hook.apply_hook, alias=fruit))
    color = aliased(Color.__mapper__, flat=True)
    joined = rows.with_join(Join(("relation", cast("Any", object())), color, None, True, color))

    assert not _same_rows(rows, RowSet.over(fruit).with_edit(partial(hook.apply_hook, alias=other)))
    assert not _same_rows(rows, RowSet.over(fruit).with_edit(lambda statement: statement))
    assert not _same_rows(rows, joined)

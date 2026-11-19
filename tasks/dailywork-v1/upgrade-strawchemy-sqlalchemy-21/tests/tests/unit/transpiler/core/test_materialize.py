"""Tests of the subquery decision and of relation and EXISTS planning, on hand-built RowSets and Projections."""

from __future__ import annotations

import typing
from dataclasses import dataclass, replace
from typing import Any, cast

from inline_snapshot import snapshot
from sqlalchemy import Lateral, Subquery, and_, inspect, or_, select
from sqlalchemy.dialects import mysql, postgresql, sqlite
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column
from sqlalchemy.sql import visitors
from sqlalchemy.sql.elements import BinaryExpression, ColumnClause
from sqlalchemy.sql.util import ClauseAdapter

from strawchemy import Strawchemy
from strawchemy.dto.strawberry import ExistsFilter, Filter, QueryNode
from strawchemy.schema.filters.inputs import TextComparison
from strawchemy.transpiler._core.level import Level, PlanContext
from strawchemy.transpiler._core.pipeline import PassBase, Pipeline, Pipelines
from strawchemy.transpiler._core.render import clause_element
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._core.rowset import OrderPriority, Projection, RowSet
from strawchemy.transpiler._executor import SyncQueryExecutor
from strawchemy.transpiler.hook import QueryHook
from tests.unit.models import Color, Group, SponsoredUser, Tag, User
from tests.utils import as_dto, format_sql

if typing.TYPE_CHECKING:
    from sqlalchemy import ClauseElement
    from sqlalchemy.engine import Dialect
    from sqlalchemy.orm.util import AliasedClass
    from sqlalchemy.sql import ColumnElement
    from sqlalchemy.sql.elements import UnaryExpression

    from strawchemy.dto.strawberry import BooleanFilterDTO
    from strawchemy.transpiler._core.plan import QueryPlan
    from strawchemy.typing import QueryNodeType, SelectOf


class _PairBase(DeclarativeBase):
    pass


class _Pair(_PairBase):
    __tablename__ = "pair"

    left_id: Mapped[int] = mapped_column(primary_key=True)
    right_id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str]


_strawchemy = Strawchemy("postgresql")


@_strawchemy.type(Color, include="all")
class _ColorType: ...


@_strawchemy.type(Group, include="all")
class _GroupType: ...


@_strawchemy.type(User, include="all")
class _UserType: ...


@_strawchemy.type(SponsoredUser, include="all")
class _SponsoredUserType: ...


@_strawchemy.filter(Color, include="all")
class _ColorFilter: ...


@_strawchemy.type(Tag, include="all")
class _TagType: ...


@_strawchemy.filter(User, include="all")
class _UserFilter: ...


@_strawchemy.filter(_Pair, include="all")
class _PairFilter: ...


def _unwrap(type_: object) -> type[Any]:
    """Returns the non-None member of an optional annotation."""
    args = [arg for arg in typing.get_args(type_) if arg is not type(None)]
    return cast("type[Any]", args[0] if args else type_)


_FRUIT_FILTER = _unwrap(_ColorFilter.__annotations__["fruits"])
_SWEETNESS_COMPARISON = _unwrap(_FRUIT_FILTER.__annotations__["sweetness"])
_DEPARTMENT_FILTER = _unwrap(_UserFilter.__annotations__["departments"])
_GROUP_FILTER = _unwrap(_UserFilter.__annotations__["group"])


class _Filters(PassBase):
    """Applies the direct filter, AND and OR branches alike, as the Filtering pass will."""

    def rows(self, level: Level, rows: RowSet) -> RowSet:
        direct = level.request.filter_split.direct
        if direct is None:
            return rows
        predicate, rows = _filter_predicate(level, direct, rows)
        return rows.with_where(predicate)


@dataclass
class _OrderByPath(PassBase):
    """Orders the level of ``node`` by the column of ``path`` and keeps two rows."""

    node: QueryNodeType
    path: QueryNodeType

    def rows(self, level: Level, rows: RowSet) -> RowSet:
        if level.node is not self.node:
            return rows
        column, rows = level.path_column(self.path, rows)
        return _paginated(rows, column.asc())


class _Hooks(PassBase):
    """Runs the query hooks of the level's node as row-stage edits."""

    def rows(self, level: Level, rows: RowSet) -> RowSet:
        for hook in level.hooks(level.node):
            rows = rows.with_edit(lambda statement, hook=hook: hook.apply_hook(statement, level.alias))
        return rows


class _PaginateByName(PassBase):
    """Orders a relation level by name and keeps two rows."""

    def rows(self, level: Level, rows: RowSet) -> RowSet:
        ordered = rows.with_order_by(OrderPriority.CLIENT, clause_element(level.alias.name).asc())
        return replace(ordered, limit=2)


@dataclass
class _HideArchived(QueryHook[Color]):
    def apply_hook(self, statement: SelectOf[Color], alias: AliasedClass[Color]) -> SelectOf[Color]:
        return statement.where(alias.name != "archived")


def _context(
    model: type[DeclarativeBase],
    dialect: Dialect | None = None,
    *,
    relation: tuple[PassBase, ...] = (),
    query_hooks: dict[QueryNodeType, list[QueryHook[Any]]] | None = None,
) -> PlanContext:
    pipelines = Pipelines(
        root=Pipeline(()), relation=Pipeline(relation), exists=Pipeline((_Filters(),)), dml=Pipeline(())
    )
    return PlanContext.create(
        model, dialect or postgresql.psycopg2.dialect(), pipelines=pipelines, query_hooks=query_hooks or {}
    )


def _request(
    model: type[DeclarativeBase], selection: QueryNodeType | None, dto_filter: BooleanFilterDTO | None = None
) -> QueryRequest:
    return QueryRequest(
        model=model,
        selection_tree=selection,
        dto_filter=dto_filter,
        order_by=(),
        distinct_on=(),
        limit=None,
        offset=None,
        allow_null=False,
    )


def _filter_predicate(level: Level, dto_filter: Filter, rows: RowSet) -> tuple[ColumnElement[bool], RowSet]:
    parts: list[ColumnElement[bool]] = []
    for value in dto_filter.and_:
        if isinstance(value, Filter):
            predicate, rows = _filter_predicate(level, value, rows)
        elif isinstance(value, ExistsFilter):
            predicate = level.plan_exists(value.dto_filter)
        else:
            column, rows = level.path_column(value.field_node, rows)
            predicate = and_(*value.to_expressions(level.context.dialect, column))  # ty: ignore[unresolved-attribute]
        parts.append(predicate)
    branches: list[ColumnElement[bool]] = []
    for branch in dto_filter.or_:
        predicate, rows = _filter_predicate(level, branch, rows)
        branches.append(predicate)
    if branches:
        parts.append(or_(*branches))
    return and_(*parts), rows


def _child(node: QueryNodeType, name: str) -> QueryNodeType:
    return node.insert_child(_unwrap(node.value.type_).__dto_field_definitions__[name])


def _root(model: type[DeclarativeBase], type_: type[Any], *fields: str) -> tuple[QueryNodeType, ...]:
    root = QueryNode.root_node(model)
    return root, *(root.insert_child(type_.__dto_field_definitions__[name]) for name in fields)


def _sql(statement: ClauseElement, dialect: Dialect | None = None) -> list[str]:
    return format_sql(str(statement.compile(dialect=dialect or postgresql.psycopg2.dialect()))).splitlines()


def _exists_sql(level: Level, dto_filter: BooleanFilterDTO, dialect: Dialect | None = None) -> list[str]:
    """Compiles the EXISTS of ``dto_filter`` in a SELECT of the level's alias."""
    return _sql(select(level.alias).where(level.plan_exists(dto_filter)), dialect)


def _page(plan: QueryPlan) -> Subquery:
    page = inspect(plan.rows.source).selectable
    assert isinstance(page, Subquery)
    return page


def _paginated(rows: RowSet, *terms: UnaryExpression[Any]) -> RowSet:
    return replace(rows.with_order_by(OrderPriority.CLIENT, *terms), limit=2)


def test_inline_keeps_single_from() -> None:
    """Rows that only filter are inlined: the statement reads ``color`` once, without a subquery."""
    root, name = _root(Color, _ColorType, "name")
    level = Level.root(_request(Color, root), _context(Color))
    rows = RowSet.over(level.alias).with_where(level.column(name) == "red")

    plan = level.materialize(rows, Projection.over(root, level.alias).with_loaded(root, "name"))

    assert plan.rows is rows
    assert _sql(plan.emit()) == snapshot(
        ["SELECT color.name,", "       color.id", "  FROM color AS color", " WHERE color.name = %(name_1)s"]
    )


def test_wrap_exports_only_read_columns() -> None:
    """A paginated root is wrapped in a page exporting only what the projection reads: its loaded column and key."""
    root, name = _root(Color, _ColorType, "name")
    level = Level.root(_request(Color, root), _context(Color))
    rows = _paginated(RowSet.over(level.alias), level.column(name).asc())

    plan = level.materialize(rows, Projection.over(root, level.alias).with_loaded(root, "name"))

    assert sorted(_page(plan).c.keys()) == ["id", "name"]
    assert _sql(plan.emit()) == snapshot(
        [
            "SELECT color.name,",
            "       color.id",
            "  FROM (",
            "        SELECT color.name AS name,",
            "               color.id AS id",
            "          FROM color AS color",
            "         ORDER BY color.name ASC",
            "         LIMIT %(param_1)s",
            "       ) AS color",
            " ORDER BY color.name ASC",
        ]
    )


def test_wrap_reuses_row_stage_to_one_join() -> None:
    """A to-one join the ordering added is joined once, in the page, and the selected relation reads it from there."""
    root, _, color = _root(Group, _GroupType, "name", "color")
    color_name = _child(color, "name")
    level = Level.root(_request(Group, root), _context(Group))
    color_name_column, rows = level.path_column(color_name, RowSet.over(level.alias))
    rows = _paginated(rows, color_name_column.asc())
    projection = level.plan_child(color, rows, Projection.over(root, level.alias).with_loaded(root, "name"))

    plan = level.materialize(rows, projection)

    assert not plan.projection.joins
    assert _sql(plan.emit()) == snapshot(
        [
            'SELECT "group".name,',
            '       "group".id,',
            '       "group".name_1,',
            '       "group".id_1,',
            '       "group".private',
            "  FROM (",
            '        SELECT "group".name AS name,',
            '               "group".id AS id,',
            "               color_1.name AS name_1,",
            "               color_1.id AS id_1,",
            "               color_1.private AS PRIVATE",
            '          FROM "group" AS "group"',
            "          LEFT OUTER JOIN color AS color_1",
            '            ON color_1.id = "group".color_id',
            "         ORDER BY color_1.name ASC",
            "         LIMIT %(param_1)s",
            '       ) AS "group"',
            ' ORDER BY "group".name_1 ASC',
        ]
    )
    assert inspect(plan.relation_entities[color]).selectable is _page(plan)


def test_wrap_exports_row_stage_aggregate() -> None:
    """An aggregate the ordering computes is computed once, in the page, and the selection reads its page column."""
    root = QueryNode.root_node(Color)
    aggregation = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits_aggregate"])
    _child(aggregation, "count")
    _child(_child(aggregation, "sum"), "sweetness")
    level = Level.root(_request(Color, root), _context(Color))
    count, sum_ = sorted(level.request.aggregate_functions(aggregation).values(), key=lambda function: function.name)
    sum_column, rows = level.aggregate(sum_, RowSet.over(level.alias))
    rows = _paginated(rows, sum_column.asc())
    count_column, projection = level.projected_aggregate(count, rows, Projection.over(root, level.alias))

    plan = level.materialize(rows, projection.with_computed(count.node, count_column))

    assert _sql(plan.emit()) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       color.count_1",
            "  FROM (",
            "        SELECT color.name AS name,",
            "               color.id AS id,",
            "               color.private AS PRIVATE,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1,",
            "                       sum(fruit_1.sweetness) AS sum_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         ORDER BY anon_1.sum_1 ASC",
            "         LIMIT %(param_1)s",
            "       ) AS color",
            " ORDER BY color.sum_1 ASC",
        ]
    )
    assert plan.column_map[count.node].table is _page(plan)
    assert plan.column_map[count.node] in plan.projection.columns


def test_wrap_recorrelates_projection_lateral() -> None:
    """A projection-stage aggregate LATERAL correlated to the root is correlated to the page instead."""
    root = QueryNode.root_node(Color)
    aggregation = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits_aggregate"])
    _child(aggregation, "count")
    level = Level.root(_request(Color, root), _context(Color))
    count = next(iter(level.request.aggregate_functions(aggregation).values()))
    rows = _paginated(RowSet.over(level.alias), clause_element(level.alias.name).asc())
    count_column, projection = level.projected_aggregate(count, rows, Projection.over(root, level.alias))

    plan = level.materialize(rows, projection.with_computed(count.node, count_column))

    lateral = plan.projection.joins["aggregate", aggregation].target
    assert isinstance(lateral, Lateral)
    tables = {element.table for element in visitors.iterate(lateral) if isinstance(element, ColumnClause)}
    assert _page(plan) in tables
    assert plan.column_map[count.node].table is lateral
    assert _sql(plan.emit()) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       anon_1.count_1",
            "  FROM (",
            "        SELECT color.name AS name,",
            "               color.id AS id,",
            "               color.private AS PRIVATE",
            "          FROM color AS color",
            "         ORDER BY color.name ASC",
            "         LIMIT %(param_1)s",
            "       ) AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.name ASC",
        ]
    )


def test_wrap_composite_primary_key() -> None:
    """The page of a model with a composite primary key exports every key column."""
    level = Level.root(_request(_Pair, None), _context(_Pair))
    rows = _paginated(RowSet.over(level.alias), clause_element(level.alias.name).asc())

    plan = level.materialize(rows, Projection.over(level.node, level.alias).with_loaded(level.node, "name"))

    assert sorted(_page(plan).c.keys()) == ["left_id", "name", "right_id"]


def test_wrap_self_referential_to_one() -> None:
    """Two aliases of one table share the page: each entity reads its own exported columns, with no extra FROM."""
    root, _, sponsor = _root(SponsoredUser, _SponsoredUserType, "name", "sponsor")
    sponsor_name = sponsor.insert_child(as_dto(_SponsoredUserType).__dto_field_definitions__["name"])
    level = Level.root(_request(SponsoredUser, root), _context(SponsoredUser))
    sponsor_name_column, rows = level.path_column(sponsor_name, RowSet.over(level.alias))
    rows = _paginated(rows, sponsor_name_column.asc())
    sponsor_alias = rows.joins["relation", sponsor].alias
    projection = level.plan_child(sponsor, rows, Projection.over(root, level.alias).with_loaded(root, "name"))

    plan = level.materialize(rows, projection)

    adapter = ClauseAdapter(_page(plan))
    for alias, entity in ((level.alias, plan.rows.source), (sponsor_alias, plan.relation_entities[sponsor])):
        expected = adapter.traverse(clause_element(alias.name))  # ty: ignore[unresolved-attribute]
        assert expected in clause_element(entity.name).proxy_set
    assert _sql(plan.emit()) == snapshot(
        [
            "SELECT sponsored_user.name,",
            "       sponsored_user.id,",
            "       sponsored_user.name_1,",
            "       sponsored_user.sponsor_id,",
            "       sponsored_user.id_1,",
            "       sponsored_user.private",
            "  FROM (",
            "        SELECT sponsored_user.name AS name,",
            "               sponsored_user.id AS id,",
            "               sponsored_user_1.name AS name_1,",
            "               sponsored_user_1.sponsor_id AS sponsor_id,",
            "               sponsored_user_1.id AS id_1,",
            "               sponsored_user_1.private AS PRIVATE",
            "          FROM sponsored_user AS sponsored_user",
            "          LEFT OUTER JOIN sponsored_user AS sponsored_user_1",
            "            ON sponsored_user.id = sponsored_user_1.sponsor_id",
            "         ORDER BY sponsored_user_1.name ASC",
            "         LIMIT %(param_1)s",
            "       ) AS sponsored_user",
            " ORDER BY sponsored_user.name_1 ASC",
        ]
    )


def test_wrap_self_referential_add_where_reads_the_root_columns() -> None:
    """An executor predicate on the model reads the root's page column, not the related alias's of the same table."""
    root, _, sponsor = _root(SponsoredUser, _SponsoredUserType, "name", "sponsor")
    sponsor_name = sponsor.insert_child(as_dto(_SponsoredUserType).__dto_field_definitions__["name"])
    level = Level.root(_request(SponsoredUser, root), _context(SponsoredUser))
    sponsor_name_column, rows = level.path_column(sponsor_name, RowSet.over(level.alias))
    rows = _paginated(rows, sponsor_name_column.asc())
    projection = level.plan_child(sponsor, rows, Projection.over(root, level.alias).with_loaded(root, "name"))
    plan = level.materialize(rows, projection)
    executor = SyncQueryExecutor(plan=plan, id_field_definitions=[])

    executor.add_where(SponsoredUser.name == "x")

    where = executor.statement().whereclause
    assert isinstance(where, BinaryExpression)
    adapter = ClauseAdapter(_page(plan))
    root_name = adapter.traverse(clause_element(level.alias.name))
    sponsor_name_on_page = adapter.traverse(sponsor_name_column)
    assert root_name in where.left.proxy_set
    assert sponsor_name_on_page not in where.left.proxy_set


def test_plan_child_hooked_relation_gets_own_join() -> None:
    """A hooked relation the rows join is joined again for the selection, its hook WHERE in the ON clause."""
    root, _, color = _root(Group, _GroupType, "name", "color")
    color_name = _child(color, "name")
    context = _context(Group, relation=(_Hooks(),), query_hooks={color: [_HideArchived()]})
    level = Level.root(_request(Group, root), context)
    color_name_column, rows = level.path_column(color_name, RowSet.over(level.alias))
    rows = rows.with_where(color_name_column == "red")

    projection = level.plan_child(color, rows, Projection.over(root, level.alias))

    join = projection.joins["relation", color]
    assert join.alias is not None
    assert join.alias is not rows.joins["relation", color].alias
    assert _sql(level.materialize(rows, projection).emit()) == snapshot(
        [
            'SELECT "group".name,',
            '       "group".tag_id,',
            '       "group".color_id,',
            '       "group".id,',
            '       "group".private,',
            "       color_1.name AS name_1,",
            "       color_1.id AS id_1,",
            "       color_1.private AS private_1",
            '  FROM "group" AS "group"',
            "  LEFT OUTER JOIN color AS color_2",
            '    ON color_2.id = "group".color_id',
            "  LEFT OUTER JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "   AND color_1.name != %(name_2)s",
            " WHERE color_2.name = %(name_3)s",
        ]
    )


def test_plan_child_attaches_paginated_relation_with_its_order_by() -> None:
    """A paginated relation is attached as a LATERAL; the projection carries its ORDER BY at CLIENT priority."""
    root, fruits = _root(Color, _ColorType, "fruits")
    level = Level.root(_request(Color, root), _context(Color, relation=(_PaginateByName(),)))

    projection = level.plan_child(fruits, RowSet.over(level.alias), Projection.over(root, level.alias))

    join = projection.joins["relation", fruits]
    assert isinstance(join.target, Lateral)
    assert join.alias is not None
    assert inspect(join.alias).selectable is join.target
    ((priority, term),) = projection.order_by
    assert priority is OrderPriority.CLIENT
    assert any(isinstance(element, ColumnClause) and element.table is join.target for element in visitors.iterate(term))


def test_plan_exists_has_no_root_copy() -> None:
    """An EXISTS on a to-many filter reads the relation only, correlated to the outer alias."""
    dto_filter = _ColorFilter(fruits=_FRUIT_FILTER(sweetness=_SWEETNESS_COMPARISON(gt=5)))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    level = Level.root(_request(Color, None, dto_filter), _context(Color))

    assert _exists_sql(level, dto_filter) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private",
            "  FROM color AS color",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "           AND fruit_1.sweetness > %(sweetness_1)s",
            "       )",
        ]
    )


def test_plan_exists_under_or_splits_at_the_relation() -> None:
    """An OR of a relation branch and a root predicate is an EXISTS on the relation OR the predicate, without copy."""
    dto_filter = _ColorFilter(
        or_=[  # ty: ignore[unknown-argument]  # input fields are generated at runtime
            _ColorFilter(fruits=_FRUIT_FILTER(sweetness=_SWEETNESS_COMPARISON(gt=5))),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
            _ColorFilter(name=TextComparison(eq="red")),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
        ]
    )
    level = Level.root(_request(Color, None, dto_filter), _context(Color))

    assert _exists_sql(level, dto_filter) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private",
            "  FROM color AS color",
            " WHERE (EXISTS (SELECT 1 FROM fruit AS fruit_1 WHERE color.id = fruit_1.color_id AND fruit_1.sweetness > %(sweetness_1)s))",
            "    OR color.name = %(name_1)s",
        ]
    )


def test_plan_exists_dml_derived_table_matches_every_primary_key() -> None:
    """A DML filter on MySQL reads the matched keys through a derived table, correlated on every key column."""
    dto_filter = _PairFilter(name=TextComparison(eq="x"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    level = Level.dml(_request(_Pair, None, dto_filter), _context(_Pair, mysql.dialect()))

    assert _exists_sql(level, dto_filter, mysql.dialect()) == snapshot(
        [
            "SELECT pair.left_id,",
            "       pair.right_id,",
            "       pair.name",
            "  FROM pair",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM (",
            "                SELECT pair_1.left_id AS left_id,",
            "                       pair_1.right_id AS right_id",
            "                  FROM pair AS pair_1",
            "                 WHERE pair_1.name = %s",
            "               ) AS dml_matched",
            "         WHERE dml_matched.left_id = pair.left_id",
            "           AND dml_matched.right_id = pair.right_id",
            "       )",
        ]
    )


class _Relations(PassBase):
    """Plans every selected relation of the level, as the Relations pass will."""

    def project(self, level: Level, rows: RowSet, projection: Projection) -> Projection:
        for child in level.request.selection.children:
            if child.value.is_relation:
                projection = level.plan_child(child, rows, projection)
        return projection


def test_wrap_rebuilds_relation_join_on_the_page() -> None:
    """A hooked relation joined by the projection of a paginated root is joined from the page entity."""
    root, _, color = _root(Group, _GroupType, "name", "color")
    context = _context(Group, relation=(_Hooks(),), query_hooks={color: [_HideArchived()]})
    level = Level.root(_request(Group, root), context)
    rows = _paginated(RowSet.over(level.alias), clause_element(level.alias.name).asc())
    projection = level.plan_child(color, rows, Projection.over(root, level.alias))

    plan = level.materialize(rows, projection)

    onclause = plan.projection.joins["relation", color].onclause
    assert inspect(onclause.parent.entity).selectable is _page(plan)  # ty: ignore[unresolved-attribute]
    assert _sql(plan.emit()) == snapshot(
        [
            'SELECT "group".name,',
            '       "group".tag_id,',
            '       "group".color_id,',
            '       "group".id,',
            '       "group".private,',
            "       color_1.name AS name_1,",
            "       color_1.id AS id_1,",
            "       color_1.private AS private_1",
            "  FROM (",
            '        SELECT "group".name AS name,',
            '               "group".tag_id AS tag_id,',
            '               "group".color_id AS color_id,',
            '               "group".id AS id,',
            '               "group".private AS PRIVATE',
            '          FROM "group" AS "group"',
            '         ORDER BY "group".name ASC',
            "         LIMIT %(param_1)s",
            '       ) AS "group"',
            "  LEFT OUTER JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "   AND color_1.name != %(name_2)s",
            ' ORDER BY "group".name ASC',
        ]
    )


def test_plan_child_reuses_nested_row_stage_join() -> None:
    """Relations the ordering joined two levels deep are read by the nested selection, not joined again."""
    root, group = _root(User, _UserType, "group")
    color = group.insert_child(as_dto(_GroupType).__dto_field_definitions__["color"])
    color_name = _child(color, "name")
    level = Level.root(_request(User, root), _context(User, relation=(_Relations(),)))
    _, rows = level.path_column(color_name, RowSet.over(level.alias))

    projection = _Relations().project(level, rows, Projection.over(root, level.alias))

    assert not projection.joins
    assert projection.entities[group] is rows.joins["relation", group].alias
    assert projection.entities[color] is rows.joins["relation", color].alias


def test_wrap_joins_cte_relation_on_the_page() -> None:
    """Without LATERAL, a paginated relation under a paginated root is a CTE joined on the page's columns."""
    root, fruits = _root(Color, _ColorType, "fruits")
    level = Level.root(_request(Color, root), _context(Color, sqlite.dialect(), relation=(_PaginateByName(),)))
    rows = _paginated(RowSet.over(level.alias), clause_element(level.alias.name).asc())
    projection = level.plan_child(fruits, rows, Projection.over(root, level.alias))

    plan = level.materialize(rows, projection)

    onclause = plan.projection.joins["relation", fruits].onclause
    assert onclause is not None
    tables = {element.table for element in visitors.iterate(onclause) if isinstance(element, ColumnClause)}
    assert _page(plan) in tables
    assert _sql(plan.emit(), sqlite.dialect()) == snapshot(
        [
            "WITH anon_1 AS (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.color_id AS color_id,",
            "               fruit_1.sweetness AS sweetness,",
            "               fruit_1.id AS id,",
            "               fruit_1.private AS PRIVATE,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.name ASC, fruit_1.id) AS rank",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.name,",
            "                  fruit_1.color_id,",
            "                  fruit_1.sweetness,",
            "                  fruit_1.id,",
            "                  fruit_1.private",
            "         ORDER BY fruit_1.name ASC",
            "       ) SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       anon_1.name AS name_1,",
            "       anon_1.color_id,",
            "       anon_1.sweetness,",
            "       anon_1.id AS id_1,",
            "       anon_1.private AS private_1",
            "  FROM (",
            "        SELECT color.name AS name,",
            "               color.id AS id,",
            "               color.private AS PRIVATE",
            "          FROM color AS color",
            "         ORDER BY color.name ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "   AND anon_1.rank <= ?",
            " ORDER BY color.name ASC,",
            "          anon_1.name ASC",
        ]
    )


def test_relation_wrap_self_referential_exports_both_aliases() -> None:
    """A paginated self-referential relation ordered through itself exports both of its anonymous aliases."""
    definitions = as_dto(_SponsoredUserType).__dto_field_definitions__
    root = QueryNode.root_node(SponsoredUser)
    sponsored = root.insert_child(definitions["sponsored"])
    sponsor = sponsored.insert_child(definitions["sponsor"])
    sponsor_name = sponsor.insert_child(definitions["name"])
    context = _context(SponsoredUser, relation=(_OrderByPath(sponsored, sponsor_name), _Relations()))
    level = Level.root(_request(SponsoredUser, root), context)

    projection = level.plan_child(sponsored, RowSet.over(level.alias), Projection.over(root, level.alias))

    lateral = projection.joins["relation", sponsored].target
    assert isinstance(lateral, Lateral)
    assert len([column for column in lateral.c if column.key.startswith("name")]) == 2
    assert inspect(projection.entities[sponsor]).selectable is lateral
    assert _sql(level.materialize(RowSet.over(level.alias), projection).emit()) == snapshot(
        [
            "SELECT sponsored_user.name,",
            "       sponsored_user.sponsor_id,",
            "       sponsored_user.id,",
            "       sponsored_user.private,",
            "       anon_1.name AS name_1,",
            "       anon_1.sponsor_id AS sponsor_id_1,",
            "       anon_1.id AS id_1,",
            "       anon_1.private AS private_1,",
            "       anon_1.name_2,",
            "       anon_1.sponsor_id_2,",
            "       anon_1.id_2,",
            "       anon_1.private_2",
            "  FROM sponsored_user AS sponsored_user",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT sponsored_user_1.name AS name,",
            "               sponsored_user_1.sponsor_id AS sponsor_id,",
            "               sponsored_user_1.id AS id,",
            "               sponsored_user_1.private AS PRIVATE,",
            "               sponsored_user_2.name AS name_2,",
            "               sponsored_user_2.sponsor_id AS sponsor_id_2,",
            "               sponsored_user_2.id AS id_2,",
            "               sponsored_user_2.private AS private_2",
            "          FROM sponsored_user AS sponsored_user_1",
            "          LEFT OUTER JOIN sponsored_user AS sponsored_user_2",
            "            ON sponsored_user_1.id = sponsored_user_2.sponsor_id",
            "         WHERE sponsored_user_1.id = sponsored_user.sponsor_id",
            "         ORDER BY sponsored_user_2.name ASC",
            "         LIMIT %(param_1)s",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY anon_1.name_2 ASC",
        ]
    )


def test_wrap_self_referential_two_levels_deep() -> None:
    """Ordering by ``sponsor.name`` and ``sponsor.sponsor.name`` exports three aliases, each entity reading its own."""
    definitions = as_dto(_SponsoredUserType).__dto_field_definitions__
    root = QueryNode.root_node(SponsoredUser)
    sponsor = root.insert_child(definitions["sponsor"])
    sponsor_name = sponsor.insert_child(definitions["name"])
    second = sponsor.insert_child(definitions["sponsor"])
    second_name = second.insert_child(definitions["name"])
    level = Level.root(_request(SponsoredUser, root), _context(SponsoredUser, relation=(_Relations(),)))
    sponsor_column, rows = level.path_column(sponsor_name, RowSet.over(level.alias))
    second_column, rows = level.path_column(second_name, rows)
    rows = _paginated(rows, sponsor_column.asc(), second_column.asc())
    projection = _Relations().project(level, rows, Projection.over(root, level.alias).with_loaded(root, "name"))

    plan = level.materialize(rows, projection)

    adapter = ClauseAdapter(_page(plan))
    pairs = [(level.alias, plan.rows.source)]
    pairs.extend((rows.joins["relation", node].alias, plan.relation_entities[node]) for node in (sponsor, second))
    for alias, entity in pairs:
        expected = adapter.traverse(clause_element(alias.name))  # ty: ignore[unresolved-attribute]
        assert expected in clause_element(entity.name).proxy_set
    assert _sql(plan.emit()) == snapshot(
        [
            "SELECT sponsored_user.name,",
            "       sponsored_user.id,",
            "       sponsored_user.name_1,",
            "       sponsored_user.sponsor_id,",
            "       sponsored_user.id_1,",
            "       sponsored_user.private,",
            "       sponsored_user.name_2,",
            "       sponsored_user.sponsor_id_1,",
            "       sponsored_user.id_2,",
            "       sponsored_user.private_1",
            "  FROM (",
            "        SELECT sponsored_user.name AS name,",
            "               sponsored_user.id AS id,",
            "               sponsored_user_1.name AS name_1,",
            "               sponsored_user_1.sponsor_id AS sponsor_id,",
            "               sponsored_user_1.id AS id_1,",
            "               sponsored_user_1.private AS PRIVATE,",
            "               sponsored_user_2.name AS name_2,",
            "               sponsored_user_2.sponsor_id AS sponsor_id_1,",
            "               sponsored_user_2.id AS id_2,",
            "               sponsored_user_2.private AS private_1",
            "          FROM sponsored_user AS sponsored_user",
            "          LEFT OUTER JOIN sponsored_user AS sponsored_user_1",
            "            ON sponsored_user.id = sponsored_user_1.sponsor_id",
            "          LEFT OUTER JOIN sponsored_user AS sponsored_user_2",
            "            ON sponsored_user_1.id = sponsored_user_2.sponsor_id",
            "         ORDER BY sponsored_user_1.name ASC,",
            "                  sponsored_user_2.name ASC",
            "         LIMIT %(param_1)s",
            "       ) AS sponsored_user",
            " ORDER BY sponsored_user.name_1 ASC,",
            "          sponsored_user.name_2 ASC",
        ]
    )


def test_wrap_rebuilds_entities_over_recorrelated_lateral() -> None:
    """Entities read from a relation LATERAL that the root wrap re-correlates are built over the rewritten LATERAL."""
    root, users = _root(Group, _GroupType, "users")
    tag = users.insert_child(as_dto(_UserType).__dto_field_definitions__["tag"])
    tag_name = tag.insert_child(as_dto(_TagType).__dto_field_definitions__["name"])
    level = Level.root(_request(Group, root), _context(Group, relation=(_OrderByPath(users, tag_name), _Relations())))
    rows = _paginated(RowSet.over(level.alias), clause_element(level.alias.name).asc())
    projection = level.plan_child(users, rows, Projection.over(root, level.alias))

    plan = level.materialize(rows, projection)

    lateral = plan.projection.joins["relation", users].target
    assert isinstance(lateral, Lateral)
    assert lateral is not projection.joins["relation", users].target
    assert inspect(plan.relation_entities[users]).selectable is lateral
    assert inspect(plan.relation_entities[tag]).selectable is lateral
    assert _sql(plan.emit()) == snapshot(
        [
            'SELECT "group".name,',
            '       "group".tag_id,',
            '       "group".color_id,',
            '       "group".id,',
            '       "group".private,',
            "       anon_1.name AS name_1,",
            "       anon_1.group_id,",
            "       anon_1.tag_id AS tag_id_1,",
            "       anon_1.id AS id_1,",
            "       anon_1.private AS private_1,",
            "       anon_1.name_2,",
            "       anon_1.id_2,",
            "       anon_1.private_2",
            "  FROM (",
            '        SELECT "group".name AS name,',
            '               "group".tag_id AS tag_id,',
            '               "group".color_id AS color_id,',
            '               "group".id AS id,',
            '               "group".private AS PRIVATE',
            '          FROM "group" AS "group"',
            '         ORDER BY "group".name ASC',
            "         LIMIT %(param_1)s",
            '       ) AS "group"',
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT user_1.name AS name,",
            "               user_1.group_id AS group_id,",
            "               user_1.tag_id AS tag_id,",
            "               user_1.id AS id,",
            "               user_1.private AS PRIVATE,",
            "               tag_1.name AS name_2,",
            "               tag_1.id AS id_2,",
            "               tag_1.private AS private_2",
            '          FROM "user" AS user_1',
            "          LEFT OUTER JOIN tag AS tag_1",
            "            ON tag_1.id = user_1.tag_id",
            '         WHERE "group".id = user_1.group_id',
            "         ORDER BY tag_1.name ASC",
            "         LIMIT %(param_2)s",
            "       ) AS anon_1",
            "    ON TRUE",
            ' ORDER BY "group".name ASC,',
            "          anon_1.name_2 ASC",
        ]
    )


def test_wrap_hooked_to_one_filtered_and_selected() -> None:
    """A hooked to-one the rows filter on is joined in the page, and again outside it with the hook WHERE."""
    root, _, color = _root(Group, _GroupType, "name", "color")
    color_name = _child(color, "name")
    context = _context(Group, relation=(_Hooks(),), query_hooks={color: [_HideArchived()]})
    level = Level.root(_request(Group, root), context)
    color_name_column, rows = level.path_column(color_name, RowSet.over(level.alias))
    rows = _paginated(rows.with_where(color_name_column == "red"), clause_element(level.alias.name).asc())
    projection = level.plan_child(color, rows, Projection.over(root, level.alias))

    assert _sql(level.materialize(rows, projection).emit()) == snapshot(
        [
            'SELECT "group".name,',
            '       "group".tag_id,',
            '       "group".color_id,',
            '       "group".id,',
            '       "group".private,',
            "       color_1.name AS name_1,",
            "       color_1.id AS id_1,",
            "       color_1.private AS private_1",
            "  FROM (",
            '        SELECT "group".name AS name,',
            '               "group".tag_id AS tag_id,',
            '               "group".color_id AS color_id,',
            '               "group".id AS id,',
            '               "group".private AS PRIVATE',
            '          FROM "group" AS "group"',
            "          LEFT OUTER JOIN color AS color_2",
            '            ON color_2.id = "group".color_id',
            "         WHERE color_2.name = %(name_2)s",
            '         ORDER BY "group".name ASC',
            "         LIMIT %(param_1)s",
            '       ) AS "group"',
            "  LEFT OUTER JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "   AND color_1.name != %(name_3)s",
            ' ORDER BY "group".name ASC',
        ]
    )


def test_plan_exists_or_across_relations_is_one_exists_per_relation() -> None:
    """An OR across two relations is an EXISTS per relation, each starting from its relation, without root copy."""
    dto_filter = _UserFilter(
        or_=[  # ty: ignore[unknown-argument]  # input fields are generated at runtime
            _UserFilter(departments=_DEPARTMENT_FILTER(name=TextComparison(eq="x"))),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
            _UserFilter(group=_GROUP_FILTER(name=TextComparison(eq="y"))),  # ty: ignore[unknown-argument]  # input fields are generated at runtime
        ]
    )
    level = Level.root(_request(User, None, dto_filter), _context(User))

    assert _exists_sql(level, dto_filter) == snapshot(
        [
            'SELECT "user".name,',
            '       "user".group_id,',
            '       "user".tag_id,',
            '       "user".id,',
            '       "user".private',
            '  FROM "user" AS "user"',
            ' WHERE (EXISTS (SELECT 1 FROM department AS department_1 JOIN user_department_join_table AS user_department_join_table_1 ON department_1.id = user_department_join_table_1.department_id WHERE "user".id = user_department_join_table_1.user_id AND department_1.name = %(name_1)s))',
            '    OR (EXISTS (SELECT 1 FROM "group" AS group_1 WHERE group_1.id = "user".group_id AND group_1.name = %(name_2)s))',
        ]
    )


def test_plan_exists_without_relation_is_the_predicate() -> None:
    """A filter that goes through no relation of the level is its predicate, without EXISTS."""
    dto_filter = _ColorFilter(name=TextComparison(eq="red"))  # ty: ignore[unknown-argument]  # input fields are generated at runtime
    level = Level.root(_request(Color, None, dto_filter), _context(Color))

    assert _sql(level.plan_exists(dto_filter)) == snapshot(["color.name = %(name_1)s"])

"""Tests for rendering row sets and plans into statements."""

from __future__ import annotations

from dataclasses import replace
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any, cast

import pytest
from inline_snapshot import snapshot
from sqlalchemy import func, select, true
from sqlalchemy.dialects import mysql, postgresql, sqlite
from sqlalchemy.orm import Load, aliased, load_only

from strawchemy.config.databases import DatabaseFeatures
from strawchemy.dto.strawberry import OrderByEnum
from strawchemy.exceptions import TranspilingError
from strawchemy.transpiler._core.plan import QueryPlan
from strawchemy.transpiler._core.render import order_terms, render_plan, render_rows, same_column
from strawchemy.transpiler._core.rowset import AggregateJoin, AliasPage, Join, OrderPriority, Projection, RowSet
from tests.unit.models import Color, Fruit
from tests.utils import format_sql

if TYPE_CHECKING:
    from sqlalchemy import Select
    from sqlalchemy.engine import Dialect
    from sqlalchemy.sql import ColumnElement

    from strawchemy.typing import QueryNodeType

POSTGRES = DatabaseFeatures(dialect="postgresql", supports_distinct_on=True, supports_null_ordering=True)
SQLITE = DatabaseFeatures(dialect="sqlite")
_DIALECTS: dict[str, Dialect] = {
    "postgresql": postgresql.psycopg2.dialect(),
    "sqlite": sqlite.dialect(),
    "mysql": mysql.dialect(),
}


def _sql(statement: Select[Any], features: DatabaseFeatures) -> list[str]:
    compiled = statement.compile(dialect=_DIALECTS[features.dialect], compile_kwargs={"literal_binds": True})
    return format_sql(str(compiled)).splitlines()


class _Node:
    def __init__(self, level: int = 0, model_field: object = None) -> None:
        self.level = level
        self.value = SimpleNamespace(model_field=model_field)


def _node(level: int = 0) -> QueryNodeType:
    return cast("QueryNodeType", _Node(level))


def _distinct(rows: RowSet, *columns: ColumnElement[Any]) -> RowSet:
    return replace(rows, distinct_on=tuple(columns))


def test_order_terms_nulls_native_and_emulated() -> None:
    """ASC_NULLS_FIRST is native with null ordering support and an IS NULL term otherwise."""
    native = order_terms(Color.__table__.c.name, OrderByEnum.ASC_NULLS_FIRST, POSTGRES)
    emulated = order_terms(Color.__table__.c.name, OrderByEnum.ASC_NULLS_FIRST, SQLITE)

    native_sql = _sql(select(Color).order_by(*native), POSTGRES)
    emulated_sql = _sql(select(Color).order_by(*emulated), SQLITE)

    assert native_sql == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private",
            "  FROM color",
            " ORDER BY color.name ASC NULLS FIRST",
        ]
    )
    assert emulated_sql == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private",
            "  FROM color",
            " ORDER BY color.name IS NULL DESC, color.name ASC",
        ]
    )


def test_render_rows_orders_by_priority() -> None:
    """Expressions added as CLIENT then HOOK render the HOOK one first."""
    alias = aliased(Color.__mapper__, name="color")
    rows = (
        RowSet.over(alias)
        .with_order_by(OrderPriority.CLIENT, alias.name.asc())
        .with_order_by(OrderPriority.HOOK, alias.id.asc())
    )

    sql = _sql(render_rows(rows, [alias.id, alias.name], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT color.id,",
            "       color.name",
            "  FROM color AS color",
            " ORDER BY color.id ASC,",
            "          color.name ASC",
        ]
    )


def test_render_rows_distinct_native_when_order_prefix() -> None:
    """DISTINCT ON is native on PostgreSQL when the ORDER BY starts with the DISTINCT ON columns."""
    alias = aliased(Color.__mapper__, name="color")
    rows = _distinct(RowSet.over(alias).with_order_by(OrderPriority.CLIENT, alias.name.asc()), alias.name)

    sql = _sql(render_rows(rows, [alias.id, alias.name], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT DISTINCT",
            "    ON (color.name) color.id,",
            "       color.name",
            "  FROM color AS color",
            " ORDER BY color.name ASC",
        ]
    )


def test_render_rows_distinct_emulated_otherwise() -> None:
    """DISTINCT ON is ranked on SQLite, and on PostgreSQL when the ORDER BY does not start with its columns."""
    alias = aliased(Color.__mapper__, name="color")
    base = RowSet.over(alias)
    by_name = _distinct(base.with_order_by(OrderPriority.CLIENT, alias.name.asc()), alias.name)
    by_id = _distinct(base.with_order_by(OrderPriority.CLIENT, alias.id.asc()), alias.name)

    sqlite_sql = _sql(render_rows(by_name, [alias.id, alias.name], SQLITE).statement, SQLITE)
    postgres_sql = _sql(render_rows(by_id, [alias.id, alias.name], POSTGRES).statement, POSTGRES)

    assert sqlite_sql == snapshot(
        [
            "SELECT anon_1.id,",
            "       anon_1.name",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               color.name AS name,",
            "               row_number() OVER (PARTITION BY color.name ORDER BY color.name ASC) AS anon_2",
            "          FROM color AS color",
            "       ) AS anon_1",
            " WHERE anon_1.anon_2 = 1",
            " ORDER BY anon_1.name ASC",
        ]
    )
    assert postgres_sql == snapshot(
        [
            "SELECT anon_1.id,",
            "       anon_1.name",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               color.name AS name,",
            "               row_number() OVER (PARTITION BY color.name ORDER BY color.id ASC) AS anon_2",
            "          FROM color AS color",
            "       ) AS anon_1",
            " WHERE anon_1.anon_2 = 1",
            " ORDER BY anon_1.id ASC",
        ]
    )


def test_render_rows_hook_order_before_client_breaks_native_distinct() -> None:
    """A HOOK ORDER BY on another column goes first, so DISTINCT ON is ranked even on PostgreSQL."""
    alias = aliased(Color.__mapper__, name="color")
    rows = _distinct(
        RowSet.over(alias)
        .with_order_by(OrderPriority.CLIENT, alias.name.asc())
        .with_order_by(OrderPriority.HOOK, alias.id.asc()),
        alias.name,
    )

    sql = _sql(render_rows(rows, [alias.id, alias.name], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT anon_1.id,",
            "       anon_1.name",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               color.name AS name,",
            "               row_number() OVER (PARTITION BY color.name ORDER BY color.id ASC, color.name ASC) AS anon_2",
            "          FROM color AS color",
            "       ) AS anon_1",
            " WHERE anon_1.anon_2 = 1",
            " ORDER BY anon_1.id ASC,",
            "          anon_1.name ASC",
        ]
    )


def test_render_rows_edit_order_by_first() -> None:
    """The ORDER BY an edit adds goes before the CLIENT ORDER BY."""
    alias = aliased(Color.__mapper__, name="color")
    rows = (
        RowSet.over(alias)
        .with_edit(lambda statement: statement.order_by(alias.id.asc()))
        .with_order_by(OrderPriority.CLIENT, alias.name.asc())
    )

    sql = _sql(render_rows(rows, [alias.id, alias.name], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT color.id,",
            "       color.name",
            "  FROM color AS color",
            " ORDER BY color.id ASC,",
            "          color.name ASC",
        ]
    )


def test_render_rows_edit_order_by_selected_for_distinct() -> None:
    """The column an edit orders by is selected when DISTINCT ON needs it."""
    alias = aliased(Color.__mapper__, name="color")
    rows = _distinct(RowSet.over(alias).with_edit(lambda statement: statement.order_by(alias.id.asc())), alias.id)

    sql = _sql(render_rows(rows, [alias.name], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT DISTINCT",
            "    ON (color.id) color.name,",
            "       color.id",
            "  FROM color AS color",
            " ORDER BY color.id ASC",
        ]
    )


def test_render_rows_order_of_clauses() -> None:
    """Edits run first, then joins by node depth, WHERE, LIMIT and OFFSET."""
    fruit = aliased(Fruit.__mapper__, name="fruit")
    shallow_color = aliased(Color.__mapper__, name="shallow")
    deep_color = aliased(Color.__mapper__, name="deep")
    deep, shallow = _node(2), _node(1)
    rows = RowSet(
        source=fruit,
        joins={
            ("relation", deep): Join(("relation", deep), deep_color, fruit.color_id == deep_color.id, True, deep_color),
            ("relation", shallow): Join(
                ("relation", shallow), shallow_color, fruit.color_id == shallow_color.id, True, shallow_color
            ),
        },
        where=(fruit.name == "apple",),
        limit=3,
        offset=1,
    ).with_edit(lambda statement: statement.where(fruit.name != "x"))

    sql = _sql(render_rows(rows, [fruit.id], POSTGRES).statement, POSTGRES)

    assert sql == snapshot(
        [
            "SELECT fruit.id",
            "  FROM fruit AS fruit",
            "  LEFT OUTER JOIN color AS shallow",
            "    ON fruit.color_id = shallow.id",
            "  LEFT OUTER JOIN color AS deep",
            "    ON fruit.color_id = deep.id",
            " WHERE fruit.name != 'x'",
            "   AND fruit.name = 'apple'",
            " LIMIT 3",
            "OFFSET 1",
        ]
    )


def test_render_plan_selects_entities_columns_and_options() -> None:
    """The plan selects the root entity, projection columns and root aggregations last, with loader options."""
    alias = aliased(Color.__mapper__, name="color")
    root = _node()
    total = func.count(alias.id).over().label("total")
    plan = QueryPlan(
        rows=RowSet.over(alias).with_where(alias.name == "red"),
        projection=Projection.over(root, alias)
        .with_loaded(root, "id", "name")
        .with_columns(alias.name)
        .with_root_aggregation(_node(), total),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    statement = render_plan(plan)

    assert _sql(statement, POSTGRES) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.name AS name__1,",
            "       count(color.id) OVER () AS total",
            "  FROM color AS color",
            " WHERE color.name = 'red'",
        ]
    )
    assert len(statement._with_options) == 2  # noqa: SLF001


def test_render_plan_selects_a_shared_entity_once() -> None:
    """Two nodes on one entity select its columns once, and each page's rank column with the projection columns."""
    color, fruit = aliased(Color.__mapper__, name="color"), aliased(Fruit.__mapper__, name="fruit")
    root, first, second = _node(), _node(1), _node(1)
    rank = func.row_number().over(order_by=fruit.name).label("fruit_rank")
    projection = Projection.over(root, color).with_page(second, AliasPage(rank, None, 2))
    plan = QueryPlan(
        rows=RowSet.over(color),
        projection=replace(projection, entities={**projection.entities, first: fruit, second: fruit}),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    statement = render_plan(plan)

    assert _sql(statement, POSTGRES) == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       fruit.name AS name_1,",
            "       fruit.color_id,",
            "       fruit.sweetness,",
            "       fruit.id AS id_1,",
            "       fruit.private AS private_1,",
            "       row_number() OVER (ORDER BY fruit.name) AS fruit_rank",
            "  FROM color AS color,",
            "       fruit AS fruit",
        ]
    )


def test_render_plan_selects_each_rank_object() -> None:
    """A rank equal to a selected column, but another object, is selected too: the executor reads it by object."""
    color, fruit = aliased(Color.__mapper__, name="color"), aliased(Fruit.__mapper__, name="fruit")
    root, first, second = _node(), _node(1), _node(1)
    first_rank = fruit.sweetness.label("rank")
    second_rank = fruit.sweetness.label("rank")
    projection = (
        Projection.over(root, color)
        .with_columns(first_rank)
        .with_page(first, AliasPage(first_rank, None, 2))
        .with_page(second, AliasPage(second_rank, None, 3))
    )
    plan = QueryPlan(
        rows=RowSet.over(color),
        projection=replace(projection, entities={**projection.entities, first: fruit, second: fruit}),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    selected = list(render_plan(plan).selected_columns)

    assert [column for column in selected if column is first_rank] == [first_rank]
    assert [column for column in selected if column is second_rank] == [second_rank]


def test_render_plan_merges_loader_options_of_a_shared_entity() -> None:
    """Nodes on one entity load the union of their keys and their hooks through one option on that entity."""
    color, fruit = aliased(Color.__mapper__, name="color"), aliased(Fruit.__mapper__, name="fruit")
    root, first, second = _node(), _node(1), _node(1)
    hook = SimpleNamespace(
        column_load_options=lambda alias: [load_only(alias.color_id)], load_relationships=lambda _: []
    )
    projection = (
        Projection.over(root, color)
        .with_loaded(first, "id", "name")
        .with_loaded(second, "id", "sweetness")
        .with_hooks(first, cast("Any", hook))
        .with_hooks(second, cast("Any", hook))
    )
    plan = QueryPlan(
        rows=RowSet.over(color),
        projection=replace(projection, entities={**projection.entities, first: fruit, second: fruit}),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    options = render_plan(plan)._with_options  # noqa: SLF001

    assert len(options) == 2
    scoped = cast("Load", options[1])
    undeferred = [
        cast("Any", load.path[-1]).key for load in scoped.context if dict(load.strategy or ()).get("deferred") is False
    ]
    assert undeferred == ["id", "name", "sweetness", "color_id"]


def test_render_plan_keeps_projection_order_by_in_insertion_order() -> None:
    """Rows terms come first sorted by priority; projection terms keep their order, outer DETERMINISTIC first."""
    alias = aliased(Color.__mapper__, name="color")
    root = _node()
    plan = QueryPlan(
        rows=RowSet.over(alias)
        .with_order_by(OrderPriority.DETERMINISTIC, alias.id.asc())
        .with_order_by(OrderPriority.CLIENT, alias.name.desc()),
        projection=Projection.over(root, alias)
        .with_order_by(OrderPriority.DETERMINISTIC, alias.id.desc())
        .with_order_by(OrderPriority.CLIENT, alias.name.asc()),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    sql = _sql(render_plan(plan), POSTGRES)

    assert sql == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       color.private",
            "  FROM color AS color",
            " ORDER BY color.name DESC,",
            "          color.id ASC,",
            "          color.id DESC,",
            "          color.name ASC",
        ]
    )


def test_render_plan_renders_aggregate_join_as_plain_join() -> None:
    """An AggregateJoin renders like any join and its column is selected unchanged."""
    fruit = aliased(Fruit.__mapper__, name="fruit")
    root, function = _node(), _node(1)
    lateral = select(func.count(Color.id).label("n")).lateral("agg")
    join = AggregateJoin(("aggregate", function), lateral, true(), True, None, {function: lateral.c.n})
    plan = QueryPlan(
        rows=RowSet.over(fruit),
        projection=Projection.over(root, fruit).with_join(join).with_columns(join.columns[function]),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    statement = render_plan(plan)

    assert _sql(statement, POSTGRES) == snapshot(
        [
            "SELECT fruit.name,",
            "       fruit.color_id,",
            "       fruit.sweetness,",
            "       fruit.id,",
            "       fruit.private,",
            "       agg.n",
            "  FROM fruit AS fruit",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT count(color.id) AS n",
            "          FROM color",
            "       ) AS agg",
            "    ON TRUE",
        ]
    )
    assert any(column is lateral.c.n for column in statement.selected_columns)


def test_render_rows_returns_final_order_by() -> None:
    """Edit-added terms come first in RenderedRows.order_by and the columns they read are selected."""
    alias = aliased(Color.__mapper__, name="color")
    rows = (
        RowSet.over(alias)
        .with_edit(lambda statement: statement.order_by(alias.id.asc()))
        .with_order_by(OrderPriority.CLIENT, alias.name.asc())
    )

    result = render_rows(rows, [alias.id], POSTGRES)

    assert [str(term.compile(dialect=_DIALECTS["postgresql"])) for term in result.order_by] == snapshot(
        ["color.id ASC", "color.name ASC"]
    )
    assert len(result.statement.selected_columns) == 2


def test_render_rows_partition_by() -> None:
    """Emulated DISTINCT ON partitions by partition_by, then by the DISTINCT ON columns."""
    fruit = aliased(Fruit.__mapper__, name="fruit")
    rows = _distinct(RowSet.over(fruit).with_order_by(OrderPriority.CLIENT, fruit.name.asc()), fruit.name)

    sql = _sql(render_rows(rows, [fruit.id], SQLITE, partition_by=[fruit.color_id]).statement, SQLITE)

    assert sql == snapshot(
        [
            "SELECT anon_1.id,",
            "       anon_1.name",
            "  FROM (",
            "        SELECT fruit.id AS id,",
            "               fruit.name AS name,",
            "               row_number() OVER (PARTITION BY fruit.color_id, fruit.name ORDER BY fruit.name ASC) AS anon_2",
            "          FROM fruit AS fruit",
            "       ) AS anon_1",
            " WHERE anon_1.anon_2 = 1",
            " ORDER BY anon_1.name ASC",
        ]
    )


def test_render_plan_rejects_unwrapped_distinct_on() -> None:
    """Rows with DISTINCT ON reaching render_plan raise TranspilingError."""
    alias = aliased(Color.__mapper__, name="color")
    plan = QueryPlan(
        rows=_distinct(RowSet.over(alias), alias.name),
        projection=Projection.over(_node(), alias),
        context=cast("Any", SimpleNamespace(db_features=POSTGRES, dialect="postgresql")),
    )

    with pytest.raises(TranspilingError):
        render_plan(plan)


def test_same_column_tells_anonymous_aliases_of_one_table_apart() -> None:
    """Two anonymous aliases of one table do not share a column, while an alias's column equals itself."""
    first, second = aliased(Color.__mapper__), aliased(Color.__mapper__)
    first_name, second_name = first.name.__clause_element__(), second.name.__clause_element__()

    assert not same_column(first_name, second_name)
    assert same_column(first_name, first.name.__clause_element__())


def test_root_entity_of_plan_without_entity_raises() -> None:
    """A plan whose projection holds no entity has no root entity: ``TranspilingError`` is raised."""
    alias = aliased(Color.__mapper__, name="color")
    plan = QueryPlan(rows=RowSet.over(alias), projection=Projection(), context=cast("Any", SimpleNamespace()))

    with pytest.raises(TranspilingError, match="no entity"):
        _ = plan.root_entity

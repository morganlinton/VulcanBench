"""Tests of CTE sharing on hand-built plans."""

from __future__ import annotations

import operator
from dataclasses import dataclass, replace
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any, cast

import pytest
from inline_snapshot import snapshot
from sqlalchemy import and_, exists, func, inspect, literal, or_, select
from sqlalchemy.dialects import sqlite
from sqlalchemy.orm import aliased
from sqlalchemy.sql import visitors
from sqlalchemy.sql.elements import BinaryExpression
from sqlalchemy.sql.selectable import CTE

from strawchemy.config.databases import DatabaseFeatures
from strawchemy.transpiler._core.plan import QueryPlan
from strawchemy.transpiler._core.render import render_plan
from strawchemy.transpiler._core.rowset import AliasPage, Join, Projection, RowSet
from strawchemy.transpiler._core.share import share_ctes
from tests.unit.models import Color, Fruit
from tests.utils import format_sql

if TYPE_CHECKING:
    from collections.abc import Callable
    from typing import TypeAlias

    from sqlalchemy import Label, Select
    from sqlalchemy.orm.util import AliasedClass
    from sqlalchemy.sql.elements import KeyedColumnElement

    from strawchemy.typing import QueryNodeType

_Edit: TypeAlias = "Callable[[Select, type[Fruit]], Select[Any]]"
_RankBy: TypeAlias = "Callable[[type[Fruit]], list[Any]]"

_SQLITE = DatabaseFeatures(dialect="sqlite")


class _Node:
    def __init__(self, level: int = 0) -> None:
        self.level = level


def _node(level: int = 0) -> QueryNodeType:
    return cast("QueryNodeType", _Node(level))


def _fruit_cte(*, with_other: bool) -> tuple[CTE, list[AliasedClass[Any]]]:
    """Builds a CTE over fruit, joined to a second alias of fruit when ``with_other``, and an entity per alias."""
    fruit = aliased(Fruit, flat=True)
    columns: list[Any] = [fruit.id, fruit.color_id]
    statement = select(*columns).select_from(fruit)
    aliases = [fruit]
    if with_other:
        other = aliased(Fruit, flat=True)
        statement = statement.add_columns(other.id, other.color_id).join(other, other.color_id == fruit.color_id)
        aliases.append(other)
    cte = statement.cte()
    return cte, [cast("AliasedClass[Any]", aliased(alias, cte)) for alias in aliases]


def _plan(*, with_other: bool) -> QueryPlan:
    """Builds a plan joining two CTEs with identical bodies to the root, and projecting every entity over them."""
    color = cast("AliasedClass[Any]", aliased(Color, name="color"))
    projection = Projection.over(_node(), color)
    for _ in range(2):
        cte, entities = _fruit_cte(with_other=with_other)
        node = _node(1)
        projection = projection.with_join(Join(("relation", node), cte, cte.c.color_id == color.id, True, entities[0]))
        for entity in entities:
            projection = replace(projection, entities={**projection.entities, _node(1): entity})
    return QueryPlan(RowSet.over(color), projection, cast("Any", SimpleNamespace(db_features=_SQLITE)))


def _unedited(statement: Select[Any], _: type[Fruit]) -> Select[Any]:
    return statement


def _no_leading_terms(_: type[Fruit]) -> list[Any]:
    return []


def _by_count(fruit: type[Fruit]) -> list[Any]:
    other = aliased(Fruit, flat=True)
    return [select(func.count(other.id)).where(other.color_id == fruit.color_id).scalar_subquery()]


def _by_any_color_name(_: type[Fruit]) -> list[Any]:
    return [aliased(Color, flat=True).name]


@dataclass(frozen=True)
class _Ranked:
    """A rank CTE of fruit and the nodes reading it, each with its page, as a CTE join builds them."""

    windows: tuple[tuple[bool, int], ...]
    """Per node, whether it ranks by descending sweetness, and its limit."""
    edit: _Edit = _unedited
    """Edits the body before it is grouped and ranked."""
    rank_by: _RankBy = _no_leading_terms
    """The terms each rank orders by before sweetness."""


def _rank_cte(ranked: _Ranked) -> tuple[CTE, AliasedClass[Any], list[KeyedColumnElement[Any]]]:
    """Builds the CTE of ``ranked``, ranked per color, the entity reading it and its rank per window."""
    fruit = aliased(Fruit, flat=True)
    statement = select(fruit.id, fruit.color_id, fruit.sweetness).where(fruit.color_id.is_not(None))
    first_descending = ranked.windows[0][0]
    statement = ranked.edit(statement, fruit).order_by(
        fruit.sweetness.desc() if first_descending else fruit.sweetness.asc()
    )
    ranks: list[Label[int]] = []
    for number, (descending, _) in enumerate(ranked.windows, 1):
        order_by = [*ranked.rank_by(fruit), fruit.sweetness.desc() if descending else fruit.sweetness.asc(), fruit.id]
        name = "rank" if len(ranked.windows) == 1 else f"rank_{number}"
        ranks.append(func.dense_rank().over(partition_by=[fruit.color_id], order_by=order_by).label(name))
    cte = statement.group_by(*statement.selected_columns).add_columns(*ranks).cte()
    return cte, cast("AliasedClass[Any]", aliased(Fruit, cte)), [cte.c[rank.name] for rank in ranks]


def _rank_plan(*ctes: _Ranked) -> QueryPlan:
    """Builds a plan joining rank CTEs of fruit to the root, by default top 2 by sweetness, then top 3 by sourness.

    Each CTE is joined once, keyed by its first node, on the bounds of all its windows.
    """
    color = cast("AliasedClass[Any]", aliased(Color, name="color"))
    projection = Projection.over(_node(), color)
    for ranked in ctes or (_Ranked(((True, 2),)), _Ranked(((False, 3),))):
        cte, entity, ranks = _rank_cte(ranked)
        nodes = [_node(1) for _ in ranked.windows]
        bounds = [rank <= limit for rank, (_, limit) in zip(ranks, ranked.windows, strict=True)]
        onclause = and_(cte.c.color_id == color.id, or_(*bounds))
        projection = projection.with_join(Join(("relation", nodes[0]), entity, onclause, True, entity))
        for node, rank, (_, limit) in zip(nodes, ranks, ranked.windows, strict=True):
            projection = replace(projection, entities={**projection.entities, node: entity})
            projection = projection.with_page(node, AliasPage(rank, None, limit))
    return QueryPlan(
        RowSet.over(color), projection, cast("Any", SimpleNamespace(db_features=_SQLITE, dialect="sqlite"))
    )


def _sql(plan: QueryPlan) -> list[str]:
    compiled = render_plan(plan).compile(dialect=sqlite.dialect(), compile_kwargs={"literal_binds": True})
    return format_sql(str(compiled)).splitlines()


def test_identical_bodies_are_shared() -> None:
    """Two CTEs with identical bodies over one alias of fruit become one CTE and an alias of it."""
    plan = share_ctes(_plan(with_other=False))

    first, second = plan.projection.joins.values()
    assert second.target.element is first.target.element  # ty: ignore[unresolved-attribute]
    assert second.target is not first.target


def test_body_selecting_two_aliases_of_one_table_is_not_shared() -> None:
    """A CTE whose body selects two aliases of fruit keeps its own CTE: its entities cannot move unambiguously."""
    plan = _plan(with_other=True)

    assert share_ctes(plan) is plan


def test_subquery_reading_a_dropped_cte_reads_the_kept_cte() -> None:
    """A WHERE subquery reading the dropped CTE reads the alias of the kept CTE that replaces it."""
    plan = _plan(with_other=False)
    dropped = list(plan.projection.joins.values())[1].target
    plan = replace(plan, rows=plan.rows.with_where(exists(select(literal(1)).select_from(dropped))))

    shared = share_ctes(plan)

    second = list(shared.projection.joins.values())[1]
    read = [element for element in visitors.iterate(shared.rows.where[0]) if isinstance(element, CTE)]
    assert any(cte is second.target for cte in read)
    assert not any(cte is dropped for cte in read)


def test_rank_ctes_differing_only_in_windows_merge() -> None:
    """Two rank CTEs differing only in their windows are one CTE ranked twice, each join keeping its own bounds."""
    plan = share_ctes(_rank_plan())

    assert _sql(plan) == snapshot(
        [
            "WITH anon_1 AS (",
            "        SELECT fruit_1.id AS id,",
            "               fruit_1.color_id AS color_id,",
            "               fruit_1.sweetness AS sweetness,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.id,",
            "                  fruit_1.color_id,",
            "                  fruit_1.sweetness",
            "         ORDER BY fruit_1.sweetness DESC",
            "       ) SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       anon_1.color_id,",
            "       anon_1.sweetness,",
            "       anon_1.id AS id_1,",
            "       anon_2.color_id AS color_id_1,",
            "       anon_2.sweetness AS sweetness_1,",
            "       anon_2.id AS id_2,",
            "       anon_1.rank_1,",
            "       anon_2.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON anon_1.color_id = color.id",
            "   AND anon_1.rank_1 <= 2",
            "  LEFT OUTER JOIN anon_1 AS anon_2",
            "    ON anon_2.color_id = color.id",
            "   AND anon_2.rank_2 <= 3",
        ]
    )
    second_node, second = list(plan.projection.joins.items())[1]
    shared_alias = inspect(second.target).selectable  # ty: ignore[unresolved-attribute]
    assert plan.projection.pages[second_node[1]].rank is shared_alias.c.rank_2


@pytest.mark.parametrize(
    "ctes",
    [
        pytest.param((_Ranked(((True, 2),)), _Ranked(((False, 3), (True, 4)))), id="single-then-shared"),
        pytest.param((_Ranked(((False, 3), (True, 4))), _Ranked(((True, 2),))), id="shared-then-single"),
        pytest.param((_Ranked(((True, 2),)), _Ranked(((False, 3),)), _Ranked(((False, 5),))), id="second-repeated"),
    ],
)
def test_merged_rank_ctes_keep_each_page_on_its_own_rank(
    ctes: tuple[_Ranked, ...], request: pytest.FixtureRequest
) -> None:
    """Merged rank CTEs, sibling ranks or a repeated body among them, give each page its own rank and bounds."""
    plan = share_ctes(_rank_plan(*ctes))

    assert (
        _sql(plan)
        == snapshot(
            {
                "single-then-shared": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_3",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "         ORDER BY fruit_1.sweetness DESC",
                    "       ) SELECT color.name,",
                    "       color.id,",
                    "       color.private,",
                    "       anon_1.color_id,",
                    "       anon_1.sweetness,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.color_id AS color_id_1,",
                    "       anon_2.sweetness AS sweetness_1,",
                    "       anon_2.id AS id_2,",
                    "       anon_1.rank_1,",
                    "       anon_2.rank_2,",
                    "       anon_2.rank_3",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON anon_1.color_id = color.id",
                    "   AND anon_1.rank_1 <= 2",
                    "  LEFT OUTER JOIN anon_1 AS anon_2",
                    "    ON anon_2.color_id = color.id",
                    "   AND (anon_2.rank_2 <= 3 OR anon_2.rank_3 <= 4)",
                ],
                "shared-then-single": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_2,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_3",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "         ORDER BY fruit_1.sweetness ASC",
                    "       ) SELECT color.name,",
                    "       color.id,",
                    "       color.private,",
                    "       anon_1.color_id,",
                    "       anon_1.sweetness,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.color_id AS color_id_1,",
                    "       anon_2.sweetness AS sweetness_1,",
                    "       anon_2.id AS id_2,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2,",
                    "       anon_2.rank_3",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON anon_1.color_id = color.id",
                    "   AND (anon_1.rank_1 <= 3 OR anon_1.rank_2 <= 4)",
                    "  LEFT OUTER JOIN anon_1 AS anon_2",
                    "    ON anon_2.color_id = color.id",
                    "   AND anon_2.rank_3 <= 2",
                ],
                "second-repeated": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "         ORDER BY fruit_1.sweetness DESC",
                    "       ) SELECT color.name,",
                    "       color.id,",
                    "       color.private,",
                    "       anon_1.color_id,",
                    "       anon_1.sweetness,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.color_id AS color_id_1,",
                    "       anon_2.sweetness AS sweetness_1,",
                    "       anon_2.id AS id_2,",
                    "       anon_3.color_id AS color_id_2,",
                    "       anon_3.sweetness AS sweetness_2,",
                    "       anon_3.id AS id_3,",
                    "       anon_1.rank_1,",
                    "       anon_2.rank_2,",
                    "       anon_3.rank_2 AS rank_2_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON anon_1.color_id = color.id",
                    "   AND anon_1.rank_1 <= 2",
                    "  LEFT OUTER JOIN anon_1 AS anon_2",
                    "    ON anon_2.color_id = color.id",
                    "   AND anon_2.rank_2 <= 3",
                    "  LEFT OUTER JOIN anon_1 AS anon_3",
                    "    ON anon_3.color_id = color.id",
                    "   AND anon_3.rank_2 <= 5",
                ],
            }
        )[request.node.callspec.id]
    )
    pages = iter(plan.projection.pages.values())
    for ranked, join in zip(ctes, plan.projection.joins.values(), strict=True):
        target = inspect(join.target).selectable  # ty: ignore[unresolved-attribute]
        bounds = [
            (bound.left, bound.right.value)
            for bound in visitors.iterate(join.onclause)
            if isinstance(bound, BinaryExpression) and bound.operator is operator.le
        ]
        for _, limit in ranked.windows:
            rank = next(pages).rank
            assert rank.table is target
            assert any(left is rank and value == limit for left, value in bounds)


def test_rank_ctes_whose_ranks_read_another_from_clause_stay_separate() -> None:
    """A rank CTE whose rank alone reads a FROM clause, which multiplies its rows, keeps its CTE."""
    plan = _rank_plan(_Ranked(((True, 2),)), _Ranked(((False, 3),), rank_by=_by_any_color_name))

    assert share_ctes(plan) is plan


def test_rank_ctes_over_a_join_merge() -> None:
    """Rank CTEs whose bodies join another table merge, the windows reading the first body's FROM clauses."""

    def joined(statement: Select[Any], fruit: type[Fruit]) -> Select[Any]:
        color = aliased(Color, flat=True)
        return statement.join(color, color.id == fruit.color_id).where(color.name != "red")

    plan = share_ctes(_rank_plan(_Ranked(((True, 2),), edit=joined), _Ranked(((False, 3),), edit=joined)))

    assert _sql(plan) == snapshot(
        [
            "WITH anon_1 AS (",
            "        SELECT fruit_1.id AS id,",
            "               fruit_1.color_id AS color_id,",
            "               fruit_1.sweetness AS sweetness,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2",
            "          FROM fruit AS fruit_1",
            "          JOIN color AS color_1",
            "            ON color_1.id = fruit_1.color_id",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "           AND color_1.name != 'red'",
            "         GROUP BY fruit_1.id,",
            "                  fruit_1.color_id,",
            "                  fruit_1.sweetness",
            "         ORDER BY fruit_1.sweetness DESC",
            "       ) SELECT color.name,",
            "       color.id,",
            "       color.private,",
            "       anon_1.color_id,",
            "       anon_1.sweetness,",
            "       anon_1.id AS id_1,",
            "       anon_2.color_id AS color_id_1,",
            "       anon_2.sweetness AS sweetness_1,",
            "       anon_2.id AS id_2,",
            "       anon_1.rank_1,",
            "       anon_2.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON anon_1.color_id = color.id",
            "   AND anon_1.rank_1 <= 2",
            "  LEFT OUTER JOIN anon_1 AS anon_2",
            "    ON anon_2.color_id = color.id",
            "   AND anon_2.rank_2 <= 3",
        ]
    )


def test_rank_ctes_with_different_bodies_stay_separate() -> None:
    """Rank CTEs whose bodies differ beyond their windows, here in a WHERE, keep a CTE each."""

    def sweet_only(statement: Select[Any], fruit: type[Fruit]) -> Select[Any]:
        return statement.where(fruit.sweetness > 5)

    plan = _rank_plan(_Ranked(((True, 2),)), _Ranked(((False, 3),), edit=sweet_only))

    assert share_ctes(plan) is plan


@pytest.mark.parametrize("first_rows", [lambda statement: statement.limit(5), lambda statement: statement.fetch(5)])
def test_rank_ctes_keeping_first_rows_ordered_differently_stay_separate(
    first_rows: Callable[[Select[Any]], Select[Any]],
) -> None:
    """Rank CTEs whose bodies keep their first rows by a LIMIT or FETCH, differently ordered, keep a CTE each."""

    def limited(statement: Select[Any], _: type[Fruit]) -> Select[Any]:
        return first_rows(statement)

    plan = _rank_plan(_Ranked(((True, 2),), edit=limited), _Ranked(((False, 3),), edit=limited))

    assert share_ctes(plan) is plan


def test_rank_ctes_with_windows_reading_a_subquery_stay_separate() -> None:
    """A rank CTE whose window reads a subquery keeps its CTE: the subquery's FROM clauses are not the body's."""
    plan = _rank_plan(_Ranked(((True, 2),)), _Ranked(((False, 3),), rank_by=_by_count))

    assert share_ctes(plan) is plan

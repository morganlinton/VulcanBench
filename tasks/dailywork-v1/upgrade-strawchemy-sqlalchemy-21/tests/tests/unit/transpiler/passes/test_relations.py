"""Tests for the ``Relations`` pass: joins of selected relations and the ORDER BY they add to their parent."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from inline_snapshot import snapshot
from sqlalchemy.dialects import postgresql

from strawchemy import Strawchemy
from strawchemy.config.databases import DatabaseFeatures
from strawchemy.dto.strawberry import QueryNode
from strawchemy.transpiler._core.level import Level, PlanContext
from strawchemy.transpiler._core.pipeline import Pipeline, Pipelines
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._passes.relations import Relations, _shares, _sibling_groups
from strawchemy.transpiler.hook import QueryHook
from tests.unit.models import Color, User
from tests.unit.transpiler.passes.utils import plan_sql
from tests.utils import as_dto

if TYPE_CHECKING:
    from strawchemy.transpiler._core.rowset import Projection, RowSet
    from strawchemy.typing import QueryNodeType

_strawchemy = Strawchemy("postgresql")


_EMPTY = Pipeline(())
_PIPELINES = Pipelines(root=_EMPTY, relation=_EMPTY, exists=_EMPTY, dml=_EMPTY)
_LATERAL = DatabaseFeatures("postgresql", supports_lateral=True)
_NO_LATERAL = DatabaseFeatures("sqlite", supports_lateral=False)


_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
_CTE_DIALECTS = pytest.mark.parametrize("dialect_name", ["sqlite", "mysql"])


@_strawchemy.type(Color, include="all")
class _ColorType: ...


@_strawchemy.type(User, include="all")
class _UserType: ...


class _HookA(QueryHook[Color]): ...


class _HookB(QueryHook[Color]): ...


def _shared_sql(query: str) -> list[str]:
    return plan_sql(query, "postgresql", literal_binds=True)


@_DIALECTS
def test_relations_nested_joined_flat(dialect_name: str) -> None:
    """A selected relation without ordering or pagination of its own is one plain LEFT OUTER JOIN."""
    lines = plan_sql("{ colors { fruits { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_relations_order_parent_by_child_keys(dialect_name: str) -> None:
    """With deterministic ordering, a relation without ordering of its own orders the parent query by its keys."""
    lines = plan_sql("{ colors { fruits { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_relations_order_outer_keys_before_nested_keys(dialect_name: str) -> None:
    """With deterministic ordering, each relation level's keys come after those of the level above it."""
    lines = plan_sql("{ colors { fruits { color { name } } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_relations_own_order_comes_before_nested_keys(dialect_name: str) -> None:
    """A relation's own ordering, carried by its LATERAL or CTE, orders the query before the relations below it."""
    lines = plan_sql("{ colors { fruits(orderBy: { name: ASC }) { color { name } } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.name AS name",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "         ORDER BY fruit_1.name ASC",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name ASC,",
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.name ASC, fruit_1.id) AS rank",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.name",
                    "         ORDER BY fruit_1.name ASC",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name ASC,",
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.name ASC, fruit_1.id) AS `rank`",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.name",
                    "         ORDER BY fruit_1.name ASC",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_relations_own_limit_uses_lateral_or_cte(dialect_name: str) -> None:
    """A relation with its own limit is a LATERAL join on postgresql, a CTE ranked by ``dense_rank`` otherwise."""
    lines = plan_sql("{ colorsPaginatedFruits { fruits(limit: 2) { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "         ORDER BY fruit_1.id ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          anon_1.id ASC",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC, fruit_1.id) AS rank",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.id ASC",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND anon_1.rank > ?",
                    "   AND anon_1.rank <= ?",
                    " ORDER BY color.id ASC,",
                    "          anon_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC, fruit_1.id) AS `rank`",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.id ASC",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND anon_1.`rank` > %s",
                    "   AND anon_1.`rank` <= %s",
                    " ORDER BY color.id ASC,",
                    "          anon_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_same_model_two_paths_gets_two_aliases(dialect_name: str) -> None:
    """A model reached from the root and through a relation is read from two aliases, correlated differently."""
    lines = plan_sql("{ colors { fruits { color { name } } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       fruit_1.id AS id_1,",
                    "       color_1.name,",
                    "       color_1.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_CTE_DIALECTS
def test_aliases_differing_in_limit_share_one_rank_cte(dialect_name: str) -> None:
    """Without LATERAL, aliases of a relation differing only in limit join one rank CTE, once, with both bounds."""
    lines = plan_sql(
        "{ colorsPaginatedFruits { a: fruits(limit: 2) { name } b: fruits(limit: 3) { name } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND (anon_1.rank_1 <= 2 OR anon_1.rank_2 <= 3)",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND (anon_1.rank_1 <= 2 OR anon_1.rank_2 <= 3)",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_CTE_DIALECTS
def test_small_page_aliases_share_one_rank_cte(dialect_name: str) -> None:
    """Without LATERAL, two small pages share one rank CTE, ranked once per alias and joined on either page."""
    lines = plan_sql(
        "{ colorsOrderedPaginatedFruits { id sourest: fruits(orderBy: { sweetness: ASC }, limit: 2) { name } "
        "sweetest: fruits(orderBy: { sweetness: DESC }, limit: 2) { name } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND (anon_1.rank_1 <= 2 OR anon_1.rank_2 <= 2)",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "   AND (anon_1.rank_1 <= 2 OR anon_1.rank_2 <= 2)",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_CTE_DIALECTS
def test_unbounded_aliases_share_one_rank_cte(dialect_name: str) -> None:
    """Without LATERAL, two orderings of every fruit share one rank CTE, joined without a page condition."""
    lines = plan_sql(
        "{ colors { id sweetFirst: fruits(orderBy: { sweetness: DESC }) { name } "
        "sourFirst: fruits(orderBy: { sweetness: ASC }) { name } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               fruit_1.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  fruit_1.sweetness",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_CTE_DIALECTS
def test_nested_selections_under_shared_rank_cte(dialect_name: str) -> None:
    """Without LATERAL, selections under aliases sharing a rank CTE are one aggregate join and one to-one join."""
    lines = plan_sql(
        "{ groupsOrderedUsers { id "
        "a: users(orderBy: { name: ASC }) { id departmentsAggregate { count } tag { name } } "
        "b: users(orderBy: { name: DESC }) { id departmentsAggregate { count } } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT user_1.id AS id,",
                    "               user_1.tag_id AS tag_id,",
                    "               user_1.group_id AS group_id,",
                    "               user_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
                    "          FROM USER AS user_1",
                    "         WHERE user_1.group_id IS NOT NULL",
                    "         GROUP BY user_1.id,",
                    "                  user_1.tag_id,",
                    "                  user_1.group_id,",
                    "                  user_1.name",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               user_2.id AS id",
                    "          FROM USER AS user_2",
                    "          JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON user_2.id = user_department_join_table_1.user_id",
                    "          JOIN department AS department_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    "         GROUP BY user_2.id",
                    '       ) SELECT "group".id,',
                    "       anon_1.id AS id_1,",
                    "       tag_1.name,",
                    "       tag_1.id AS id_2,",
                    "       anon_1.id AS group__users__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_1,",
                    "       anon_1.id AS group__users__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_2,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN anon_1",
                    '    ON "group".id = anon_1.group_id',
                    "  LEFT OUTER JOIN tag AS tag_1",
                    "    ON tag_1.id = anon_1.tag_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON anon_1.id = anon_2.id",
                    ' ORDER BY "group".id ASC,',
                    "          tag_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT user_1.id AS id,",
                    "               user_1.tag_id AS tag_id,",
                    "               user_1.group_id AS group_id,",
                    "               user_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
                    "          FROM USER AS user_1",
                    "         WHERE user_1.group_id IS NOT NULL",
                    "         GROUP BY user_1.id,",
                    "                  user_1.tag_id,",
                    "                  user_1.group_id,",
                    "                  user_1.name",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               user_2.id AS id",
                    "          FROM USER AS user_2",
                    "         INNER JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON user_2.id = user_department_join_table_1.user_id",
                    "         INNER JOIN department AS department_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    "         GROUP BY user_2.id",
                    "       ) SELECT `group`.id,",
                    "       anon_1.id AS id_1,",
                    "       tag_1.name,",
                    "       tag_1.id AS id_2,",
                    "       anon_1.id AS group__users__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_1,",
                    "       anon_1.id AS group__users__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_2,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM `group` AS `group`",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON `group`.id = anon_1.group_id",
                    "  LEFT OUTER JOIN tag AS tag_1",
                    "    ON tag_1.id = anon_1.tag_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON anon_1.id = anon_2.id",
                    " ORDER BY `group`.id ASC,",
                    "          tag_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_CTE_DIALECTS
def test_two_paths_share_one_rank_cte_without_lateral(dialect_name: str) -> None:
    """Without LATERAL, a relation and the same relation reached through another path share one rank CTE."""
    lines = plan_sql(
        "{ colors { id fruits(orderBy: { sweetness: DESC }) { sweetness "
        "color { fruits(orderBy: { sweetness: ASC }) { sweetness } } } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.sweetness AS sweetness,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.sweetness,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.sweetness DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.sweetness,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.sweetness AS sweetness_1,",
                    "       anon_2.id AS id_3",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_1 AS anon_2",
                    "    ON color_1.id = anon_2.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.sweetness DESC,",
                    "          color_1.id ASC,",
                    "          anon_2.sweetness ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.sweetness AS sweetness,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness DESC, fruit_1.id) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.sweetness ASC, fruit_1.id) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.sweetness,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.sweetness DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.sweetness,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.sweetness AS sweetness_1,",
                    "       anon_2.id AS id_3",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_1 AS anon_2",
                    "    ON color_1.id = anon_2.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.sweetness DESC,",
                    "          color_1.id ASC,",
                    "          anon_2.sweetness ASC",
                ],
            }
        )[dialect_name]
    )


def _fruits(
    root: QueryNodeType, *, limit: int | None = None, offset: int | None = None, distinct_on: tuple[object, ...] = ()
) -> QueryNodeType:
    node = root.insert_child(as_dto(_ColorType).__dto_field_definitions__["fruits"])
    node.metadata.data.relation_filter = type(node.metadata.data.relation_filter)(
        limit=limit, offset=offset, distinct_on=distinct_on
    )
    return node


def _level(root: QueryNodeType, query_hooks: dict[QueryNodeType, list[QueryHook[Color]]] | None = None) -> Level:
    request = QueryRequest(
        model=root.value.model,
        selection_tree=root,
        dto_filter=None,
        order_by=(),
        distinct_on=(),
        limit=None,
        offset=None,
        allow_null=False,
    )
    context = PlanContext.create(
        request.model, postgresql.psycopg2.dialect(), pipelines=_PIPELINES, query_hooks=query_hooks or {}
    )
    return Level.root(request, context)


def test_sibling_groups_group_by_relationship() -> None:
    """Aliases of one relationship form a group, in selection order; another relationship is a group of its own."""
    root = QueryNode.root_node(User)
    group_a = root.insert_child(as_dto(_UserType).__dto_field_definitions__["group"])
    tag = root.insert_child(as_dto(_UserType).__dto_field_definitions__["tag"])
    group_b = root.insert_child(as_dto(_UserType).__dto_field_definitions__["group"])
    group_b.metadata.data.relation_filter = type(group_b.metadata.data.relation_filter)(limit=1)

    assert _sibling_groups(_level(root)) == [(group_a, group_b), (tag,)]


def test_distinct_on_alias_leaves_the_group() -> None:
    """An alias with DISTINCT ON is a group of one, beside the group of the other aliases."""
    root = QueryNode.root_node(Color)
    first = _fruits(root, limit=1)
    distinct = _fruits(root, distinct_on=("name",))
    last = _fruits(root, limit=2)

    assert _sibling_groups(_level(root)) == [(first, last), (distinct,)]


def test_differing_hooks_leave_the_group() -> None:
    """An alias whose hooks differ from those of the group's first alias is a group of one."""
    root = QueryNode.root_node(Color)
    first, hooked, last = _fruits(root, limit=1), _fruits(root, limit=2), _fruits(root, limit=3)
    hooks: dict[QueryNodeType, list[QueryHook[Color]]] = {first: [_HookA()], hooked: [_HookB()], last: [_HookA()]}

    assert _sibling_groups(_level(root, hooks)) == [(first, last), (hooked,)]


def test_group_of_one_does_not_share() -> None:
    """A lone alias is planned on its own, whatever the database."""
    root = QueryNode.root_node(Color)

    assert not _shares((_fruits(root),), _NO_LATERAL)
    assert not _shares((_fruits(root),), _LATERAL)


def test_shares_without_lateral_always() -> None:
    """Without LATERAL, any group of several aliases shares one ranked CTE, bounded or not."""
    root = QueryNode.root_node(Color)

    assert _shares((_fruits(root, limit=1), _fruits(root, limit=1)), _NO_LATERAL)
    assert _shares((_fruits(root), _fruits(root)), _NO_LATERAL)


@pytest.mark.parametrize(
    ("pages", "expected"),
    [
        pytest.param([(2, None), (2, None)], False, id="2x2"),
        pytest.param([(4, None), (4, None)], False, id="4x4"),
        pytest.param([(5, None), (5, None)], True, id="5x5"),
        pytest.param([(2, None), (None, None)], True, id="unbounded-alias"),
        pytest.param([(None, None), (None, None)], True, id="unbounded"),
        pytest.param([(1, 1), (2, None)], False, id="offset-counts"),
        pytest.param([(2, None), (2, None), (2, None)], False, id="three-8"),
        pytest.param([(3, None), (3, None), (2, None)], True, id="three-18"),
    ],
)
def test_shares_on_lateral_unless_small_pages(pages: list[tuple[int | None, int | None]], expected: bool) -> None:
    """With LATERAL, a group stays separate only when every alias is bounded and the pages multiply to at most 16."""
    root = QueryNode.root_node(Color)
    group = tuple(_fruits(root, limit=limit, offset=offset) for limit, offset in pages)

    assert _shares(group, _LATERAL) is expected


def test_project_plans_a_sharing_group_at_its_first_member(monkeypatch: pytest.MonkeyPatch) -> None:
    """A sharing group is planned once, where its first alias is selected; other relations keep their position."""
    root = QueryNode.root_node(User)
    first = root.insert_child(as_dto(_UserType).__dto_field_definitions__["group"])
    tag = root.insert_child(as_dto(_UserType).__dto_field_definitions__["tag"])
    last = root.insert_child(as_dto(_UserType).__dto_field_definitions__["group"])
    last.metadata.data.relation_filter = type(last.metadata.data.relation_filter)(limit=1)
    calls: list[tuple[str, tuple[QueryNodeType, ...]]] = []

    def plan_siblings(_: Level, nodes: tuple[QueryNodeType, ...], __: RowSet, projection: Projection) -> Projection:
        calls.append(("siblings", nodes))
        return projection

    def plan_child(_: Level, node: QueryNodeType, __: RowSet, projection: Projection) -> Projection:
        calls.append(("child", (node,)))
        return projection

    monkeypatch.setattr(Level, "plan_siblings", plan_siblings)
    monkeypatch.setattr(Level, "plan_child", plan_child)

    Relations().project(_level(root), object(), object())  # ty: ignore[invalid-argument-type]

    assert calls == [("siblings", (first, last)), ("child", (tag,))]


def test_project_plans_equal_siblings_together(monkeypatch: pytest.MonkeyPatch) -> None:
    """Two aliases equal as nodes both reach ``plan_siblings``: none is dropped as a follower."""
    root = QueryNode.root_node(Color)
    first, second = _fruits(root), _fruits(root)
    calls: list[tuple[QueryNodeType, ...]] = []

    def plan_siblings(_: Level, nodes: tuple[QueryNodeType, ...], __: RowSet, projection: Projection) -> Projection:
        calls.append(nodes)
        return projection

    monkeypatch.setattr(Level, "plan_siblings", plan_siblings)

    Relations().project(_level(root), object(), object())  # ty: ignore[invalid-argument-type]

    assert len(calls) == 1
    assert [id(node) for node in calls[0]] == [id(first), id(second)]


def test_unbounded_aliases_share_one_lateral() -> None:
    """Two orderings of every fruit read ``fruit`` once, through one LATERAL ranking it once per alias."""
    lines = _shared_sql(
        "{ colors { id sweetFirst: fruits(orderBy: { sweetness: DESC }) { name } "
        "sourFirst: fruits(orderBy: { sweetness: ASC }) { name } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.id AS id,",
            "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_2",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ]
    )


def test_small_page_aliases_stay_separate() -> None:
    """Two pages of 2 fruits multiply to 4 rows at most, so each keeps its own index-backed LATERAL."""
    lines = _shared_sql(
        "{ colorsOrderedPaginatedFruits { id sourest: fruits(orderBy: { sweetness: ASC }, limit: 2) { name } "
        "sweetest: fruits(orderBy: { sweetness: DESC }, limit: 2) { name } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1,",
            "       anon_2.name AS name_1,",
            "       anon_2.id AS id_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.id AS id,",
            "               fruit_1.sweetness AS sweetness",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "         ORDER BY fruit_1.sweetness ASC",
            "         LIMIT 2",
            "        OFFSET 0",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_2.name AS name,",
            "               fruit_2.id AS id,",
            "               fruit_2.sweetness AS sweetness",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "         ORDER BY fruit_2.sweetness DESC",
            "         LIMIT 2",
            "        OFFSET 0",
            "       ) AS anon_2",
            "    ON TRUE",
            " ORDER BY color.id ASC,",
            "          anon_1.sweetness ASC,",
            "          anon_2.sweetness DESC",
        ]
    )


@pytest.mark.parametrize("second_page", ["limit: 2", "limit: 2, offset: 1"])
def test_small_page_aliases_next_to_their_aggregate_stay_separate(second_page: str) -> None:
    """Small pages next to the relation's aggregate keep a LATERAL per distinct page, beside the aggregate's own."""
    lines = _shared_sql(
        f"{{ colorsOrderedPaginatedFruits {{ id a: fruits(limit: 2) {{ name }} b: fruits({second_page}) {{ name }} "
        "fruitsAggregate { count } } }"
    )

    assert (
        lines
        == snapshot(
            {
                "limit: 2": [
                    "SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.count_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "         ORDER BY fruit_1.id ASC",
                    "         LIMIT 2",
                    "        OFFSET 0",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  JOIN LATERAL (",
                    "        SELECT count(*) AS count_1",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "       ) AS anon_2",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          anon_1.id ASC",
                ],
                "limit: 2, offset: 1": [
                    "SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.name AS name_1,",
                    "       anon_2.id AS id_2,",
                    "       anon_3.count_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "         ORDER BY fruit_1.id ASC",
                    "         LIMIT 2",
                    "        OFFSET 0",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_2.name AS name,",
                    "               fruit_2.id AS id",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "         ORDER BY fruit_2.id ASC",
                    "         LIMIT 2",
                    "        OFFSET 1",
                    "       ) AS anon_2",
                    "    ON TRUE",
                    "  JOIN LATERAL (",
                    "        SELECT count(*) AS count_1",
                    "          FROM fruit AS fruit_3",
                    "         WHERE color.id = fruit_3.color_id",
                    "       ) AS anon_3",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          anon_1.id ASC,",
                    "          anon_2.id ASC",
                ],
            }
        )[second_page]
    )


def test_bigger_pages_share_with_page_filter() -> None:
    """Pages multiplying to more than 16 rows share one LATERAL, which keeps the rows inside either page."""
    lines = _shared_sql(
        "{ colorsOrderedPaginatedFruits { id first: fruits(orderBy: { id: ASC }, limit: 5) { id } "
        "next: fruits(orderBy: { id: ASC }, limit: 5, offset: 5) { id } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.id AS id_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT anon_2.id AS id,",
            "               anon_2.rank_1 AS rank_1,",
            "               anon_2.rank_2 AS rank_2",
            "          FROM (",
            "                SELECT fruit_1.id AS id,",
            "                       row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_1,",
            "                       row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_2",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_2",
            "         WHERE anon_2.rank_1 <= 5",
            "            OR anon_2.rank_2 > 5",
            "           AND anon_2.rank_2 <= 10",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ]
    )


@pytest.mark.parametrize("unbounded", ["limit: null", "limit: null, offset: 0"])
def test_one_unbounded_alias_drops_the_page_filter(unbounded: str) -> None:
    """An alias reading every fruit, from an offset of 0 or none, keeps every ranked row: no page filter."""
    lines = _shared_sql(
        "{ colorsOrderedPaginatedFruits { id topTwo: fruits(orderBy: { sweetness: DESC }, limit: 2) { name } "
        f"all: fruits({unbounded}) {{ name }} }} }}"
    )

    assert (
        lines
        == snapshot(
            {
                "limit: null": [
                    "SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    " ORDER BY color.id ASC",
                ],
                "limit: null, offset: 0": [
                    "SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    " ORDER BY color.id ASC",
                ],
            }
        )[unbounded]
    )


def test_nested_selections_under_shared_aliases() -> None:
    """Selections under shared aliases are planned on the shared alias: one aggregate join, one to-one join."""
    lines = _shared_sql(
        "{ groupsOrderedUsers { id "
        "a: users(orderBy: { name: ASC }) { id departmentsAggregate { count } tag { name } } "
        "b: users(orderBy: { name: DESC }) { id departmentsAggregate { count } } } }"
    )

    assert lines == snapshot(
        [
            'SELECT "group".id,',
            "       anon_1.id AS id_1,",
            "       tag_1.name,",
            "       tag_1.id AS id_2,",
            "       anon_1.id AS group__users__id,",
            "       anon_2.count_1,",
            "       anon_1.id AS group__users__id,",
            "       anon_2.count_1 AS count_1__1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            '  FROM "group" AS "group"',
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT user_1.id AS id,",
            "               user_1.tag_id AS tag_id,",
            "               row_number() OVER (ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
            '          FROM "user" AS user_1',
            '         WHERE "group".id = user_1.group_id',
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN tag AS tag_1",
            "    ON tag_1.id = anon_1.tag_id",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM department AS department_1",
            "          JOIN user_department_join_table AS user_department_join_table_1",
            "            ON department_1.id = user_department_join_table_1.department_id",
            "         WHERE anon_1.id = user_department_join_table_1.user_id",
            "       ) AS anon_2",
            "    ON TRUE",
            ' ORDER BY "group".id ASC,',
            "          tag_1.id ASC",
        ]
    )


def test_aggregate_under_one_shared_alias() -> None:
    """An aggregate selected under one shared alias only is joined once, on the shared alias."""
    lines = _shared_sql(
        "{ groupsOrderedUsers { id a: users(orderBy: { name: ASC }) { id departmentsAggregate { count } } "
        "b: users(orderBy: { name: DESC }) { id } } }"
    )

    assert lines == snapshot(
        [
            'SELECT "group".id,',
            "       anon_1.id AS id_1,",
            "       anon_1.id AS group__users__id,",
            "       anon_2.count_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            '  FROM "group" AS "group"',
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT user_1.id AS id,",
            "               row_number() OVER (ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
            '          FROM "user" AS user_1',
            '         WHERE "group".id = user_1.group_id',
            "       ) AS anon_1",
            "    ON TRUE",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM department AS department_1",
            "          JOIN user_department_join_table AS user_department_join_table_1",
            "            ON department_1.id = user_department_join_table_1.department_id",
            "         WHERE anon_1.id = user_department_join_table_1.user_id",
            "       ) AS anon_2",
            "    ON TRUE",
            ' ORDER BY "group".id ASC',
        ]
    )


def test_offset_without_limit_page() -> None:
    """An offset without limit keeps the ranks after the offset, and bounds the shared rows like a limit."""
    lines = _shared_sql(
        "{ colorsOrderedPaginatedFruits { id a: fruits(offset: 2, limit: null) { name } "
        "b: fruits(limit: 1) { name } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT anon_2.name AS name,",
            "               anon_2.id AS id,",
            "               anon_2.rank_1 AS rank_1,",
            "               anon_2.rank_2 AS rank_2",
            "          FROM (",
            "                SELECT fruit_1.name AS name,",
            "                       fruit_1.id AS id,",
            "                       row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_1,",
            "                       row_number() OVER (ORDER BY fruit_1.id ASC) AS rank_2",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_2",
            "         WHERE anon_2.rank_1 > 2",
            "            OR anon_2.rank_2 <= 1",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ]
    )


def test_null_ordering_window() -> None:
    """A rank window orders on the terms of ``order_terms``, null placement included."""
    lines = _shared_sql(
        "{ colors { id a: fruits(orderBy: { sweetness: ASC_NULLS_FIRST }) { name } "
        "b: fruits(orderBy: { name: DESC }) { name } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.id AS id,",
            "               row_number() OVER (ORDER BY fruit_1.sweetness ASC NULLS FIRST, fruit_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY fruit_1.name DESC, fruit_1.id ASC) AS rank_2",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ]
    )


@pytest.mark.allow_duplicate_reads(
    reason="an alias ordered through a join of its own is not compared for sharing and keeps its own read"
)
def test_aliases_selecting_other_rows_stay_separate() -> None:
    """Aliases whose rows differ in more than their order and page keep one LATERAL each."""
    lines = _shared_sql(
        "{ colors { id a: fruits(orderBy: { color: { name: ASC } }) { id } "
        "b: fruits(orderBy: { sweetness: DESC }) { id } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.id AS id_1,",
            "       anon_2.id AS id_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.id AS id,",
            "               color_1.name AS name",
            "          FROM fruit AS fruit_1",
            "          LEFT OUTER JOIN color AS color_1",
            "            ON color_1.id = fruit_1.color_id",
            "         WHERE color.id = fruit_1.color_id",
            "         ORDER BY color_1.name ASC",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_2.id AS id,",
            "               fruit_2.sweetness AS sweetness",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "         ORDER BY fruit_2.sweetness DESC",
            "       ) AS anon_2",
            "    ON TRUE",
            " ORDER BY color.id ASC,",
            "          anon_1.name ASC,",
            "          anon_2.sweetness DESC",
        ]
    )


@_CTE_DIALECTS
@pytest.mark.allow_duplicate_reads(
    reason="an alias ordered through a join of its own is not compared for sharing and keeps its own read"
)
def test_aliases_selecting_other_rows_keep_one_rank_cte_each(dialect_name: str) -> None:
    """Without LATERAL, aliases whose rows differ in more than their order and page keep one rank CTE each."""
    lines = plan_sql(
        "{ colors { id a: fruits(orderBy: { color: { name: ASC } }) { id } "
        "b: fruits(orderBy: { sweetness: DESC }) { id } } }",
        dialect_name,
        literal_binds=True,
    )

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               color_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY color_1.name ASC, fruit_1.id) AS rank",
                    "          FROM fruit AS fruit_1",
                    "          LEFT OUTER JOIN color AS color_1",
                    "            ON color_1.id = fruit_1.color_id",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  color_1.name",
                    "         ORDER BY color_1.name ASC",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.color_id AS color_id,",
                    "               fruit_2.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.sweetness DESC, fruit_2.id) AS rank",
                    "          FROM fruit AS fruit_2",
                    "         WHERE fruit_2.color_id IS NOT NULL",
                    "         GROUP BY fruit_2.id,",
                    "                  fruit_2.color_id,",
                    "                  fruit_2.sweetness",
                    "         ORDER BY fruit_2.sweetness DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON color.id = anon_2.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name ASC,",
                    "          anon_2.sweetness DESC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               color_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY color_1.name ASC, fruit_1.id) AS `rank`",
                    "          FROM fruit AS fruit_1",
                    "          LEFT OUTER JOIN color AS color_1",
                    "            ON color_1.id = fruit_1.color_id",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.id,",
                    "                  fruit_1.color_id,",
                    "                  color_1.name",
                    "         ORDER BY color_1.name ASC",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.color_id AS color_id,",
                    "               fruit_2.sweetness AS sweetness,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.sweetness DESC, fruit_2.id) AS `rank`",
                    "          FROM fruit AS fruit_2",
                    "         WHERE fruit_2.color_id IS NOT NULL",
                    "         GROUP BY fruit_2.id,",
                    "                  fruit_2.color_id,",
                    "                  fruit_2.sweetness",
                    "         ORDER BY fruit_2.sweetness DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       anon_2.id AS id_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON color.id = anon_2.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name ASC,",
                    "          anon_2.sweetness DESC",
                ],
            }
        )[dialect_name]
    )


def test_nested_relations_under_shared_aliases() -> None:
    """A to-one and a to-many relation selected under both shared aliases are each joined once, on the shared alias."""
    to_one_lines = _shared_sql(
        "{ colors { id a: fruits(orderBy: { sweetness: ASC }) { id color { name } } "
        "b: fruits(orderBy: { sweetness: DESC }) { id color { name } } } }"
    )
    to_many_lines = _shared_sql(
        "{ groupsOrderedUsers { id a: users(orderBy: { name: ASC }) { id departments { id } } "
        "b: users(orderBy: { name: DESC }) { id departments { id } } } }"
    )

    assert to_one_lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.id AS id_1,",
            "       color_1.name,",
            "       color_1.id AS id_2,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.id AS id,",
            "               fruit_1.color_id AS color_id,",
            "               row_number() OVER (ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_2",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN color AS color_1",
            "    ON color_1.id = anon_1.color_id",
            " ORDER BY color.id ASC,",
            "          color_1.id ASC,",
            "          color_1.id ASC",
        ]
    )
    assert to_many_lines == snapshot(
        [
            'SELECT "group".id,',
            "       anon_1.id AS id_1,",
            "       department_1.id AS id_2,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            '  FROM "group" AS "group"',
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT user_1.id AS id,",
            "               row_number() OVER (ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
            '          FROM "user" AS user_1',
            '         WHERE "group".id = user_1.group_id',
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN (user_department_join_table AS user_department_join_table_1 JOIN department AS department_1 ON department_1.id = user_department_join_table_1.department_id)",
            "    ON anon_1.id = user_department_join_table_1.user_id",
            ' ORDER BY "group".id ASC,',
            "          department_1.id ASC,",
            "          department_1.id ASC",
        ]
    )


_DEEP_UNDER_ALIASES = (
    "{ groupsOrderedUsers { id "
    "a: users(orderBy: { name: ASC }) { id tag { id groups { id color { name } } groupsAggregate { count } } } "
    "b: users(orderBy: { name: DESC }) { id tag { id groups { id color { name } } groupsAggregate { count } } } } }"
)


def test_deep_relations_under_shared_aliases() -> None:
    """Relations and aggregates two and three levels under both shared aliases are each joined once."""
    lines = _shared_sql(_DEEP_UNDER_ALIASES)

    assert lines == snapshot(
        [
            'SELECT "group".id,',
            "       anon_1.id AS id_1,",
            "       tag_1.id AS id_2,",
            "       group_1.id AS id_3,",
            "       color_1.name,",
            "       color_1.id AS id_4,",
            "       tag_1.id AS users__tag__id,",
            "       anon_2.count_1,",
            "       tag_1.id AS users__tag__id,",
            "       anon_2.count_1 AS count_1__1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            '  FROM "group" AS "group"',
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT user_1.id AS id,",
            "               user_1.tag_id AS tag_id,",
            "               row_number() OVER (ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
            '          FROM "user" AS user_1',
            '         WHERE "group".id = user_1.group_id',
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN tag AS tag_1",
            "    ON tag_1.id = anon_1.tag_id",
            '  LEFT OUTER JOIN "group" AS group_1',
            "    ON tag_1.id = group_1.tag_id",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            '          FROM "group" AS group_2',
            "         WHERE tag_1.id = group_2.tag_id",
            "       ) AS anon_2",
            "    ON TRUE",
            "  LEFT OUTER JOIN color AS color_1",
            "    ON color_1.id = group_1.color_id",
            ' ORDER BY "group".id ASC,',
            "          tag_1.id ASC,",
            "          group_1.id ASC,",
            "          color_1.id ASC,",
            "          tag_1.id ASC,",
            "          group_1.id ASC,",
            "          color_1.id ASC",
        ]
    )


@_CTE_DIALECTS
def test_deep_relations_under_shared_rank_cte(dialect_name: str) -> None:
    """Without LATERAL, relations and aggregates deep under aliases sharing a rank CTE are each joined once."""
    lines = plan_sql(_DEEP_UNDER_ALIASES, dialect_name, literal_binds=True)

    assert (
        lines
        == snapshot(
            {
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT user_1.id AS id,",
                    "               user_1.tag_id AS tag_id,",
                    "               user_1.group_id AS group_id,",
                    "               user_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
                    "          FROM USER AS user_1",
                    "         WHERE user_1.group_id IS NOT NULL",
                    "         GROUP BY user_1.id,",
                    "                  user_1.tag_id,",
                    "                  user_1.group_id,",
                    "                  user_1.name",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               group_2.tag_id AS tag_id",
                    '          FROM "group" AS group_2',
                    "         WHERE group_2.tag_id IS NOT NULL",
                    "         GROUP BY group_2.tag_id",
                    '       ) SELECT "group".id,',
                    "       anon_1.id AS id_1,",
                    "       tag_1.id AS id_2,",
                    "       group_1.id AS id_3,",
                    "       color_1.name,",
                    "       color_1.id AS id_4,",
                    "       tag_1.id AS users__tag__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_1,",
                    "       tag_1.id AS users__tag__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_2,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN anon_1",
                    '    ON "group".id = anon_1.group_id',
                    "  LEFT OUTER JOIN tag AS tag_1",
                    "    ON tag_1.id = anon_1.tag_id",
                    '  LEFT OUTER JOIN "group" AS group_1',
                    "    ON tag_1.id = group_1.tag_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON tag_1.id = anon_2.tag_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = group_1.color_id",
                    ' ORDER BY "group".id ASC,',
                    "          tag_1.id ASC,",
                    "          group_1.id ASC,",
                    "          color_1.id ASC,",
                    "          tag_1.id ASC,",
                    "          group_1.id ASC,",
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT user_1.id AS id,",
                    "               user_1.tag_id AS tag_id,",
                    "               user_1.group_id AS group_id,",
                    "               user_1.name AS name,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name ASC, user_1.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY user_1.group_id ORDER BY user_1.name DESC, user_1.id ASC) AS rank_2",
                    "          FROM USER AS user_1",
                    "         WHERE user_1.group_id IS NOT NULL",
                    "         GROUP BY user_1.id,",
                    "                  user_1.tag_id,",
                    "                  user_1.group_id,",
                    "                  user_1.name",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               group_2.tag_id AS tag_id",
                    "          FROM `group` AS group_2",
                    "         WHERE group_2.tag_id IS NOT NULL",
                    "         GROUP BY group_2.tag_id",
                    "       ) SELECT `group`.id,",
                    "       anon_1.id AS id_1,",
                    "       tag_1.id AS id_2,",
                    "       group_1.id AS id_3,",
                    "       color_1.name,",
                    "       color_1.id AS id_4,",
                    "       tag_1.id AS users__tag__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_1,",
                    "       tag_1.id AS users__tag__id,",
                    "       coalesce(anon_2.count_1, 0) AS coalesce_2,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM `group` AS `group`",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON `group`.id = anon_1.group_id",
                    "  LEFT OUTER JOIN tag AS tag_1",
                    "    ON tag_1.id = anon_1.tag_id",
                    "  LEFT OUTER JOIN `group` AS group_1",
                    "    ON tag_1.id = group_1.tag_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON tag_1.id = anon_2.tag_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = group_1.color_id",
                    " ORDER BY `group`.id ASC,",
                    "          tag_1.id ASC,",
                    "          group_1.id ASC,",
                    "          color_1.id ASC,",
                    "          tag_1.id ASC,",
                    "          group_1.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


_ALIASED_UNDER_ONE_ALIAS = (
    "{ colors { id a: fruits(orderBy: { id: ASC }) { id color { x: fruits { id } y: fruits(orderBy: { name: ASC }) "
    "{ id } } } b: fruits(orderBy: { id: DESC }) { id color { fruits { name } } } } }"
)


@pytest.mark.allow_duplicate_reads(
    reason="a relation that a shared alias also selects through aliases of its own keeps one read per alias"
)
@_DIALECTS
def test_relation_aliased_under_one_shared_alias_keeps_its_read(dialect_name: str) -> None:
    """A relation that one shared alias selects under aliases of its own is joined, for the other alias, on its own."""
    lines = plan_sql(_ALIASED_UNDER_ONE_ALIAS, dialect_name, literal_binds=True)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.id AS id_3,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_4,",
                    "       anon_2.rank_1,",
                    "       anon_2.rank_2,",
                    "       anon_1.rank_1 AS rank_1_1,",
                    "       anon_1.rank_2 AS rank_2_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.color_id AS color_id,",
                    "               row_number() OVER (ORDER BY fruit_2.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_2.id DESC, fruit_2.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color.id = fruit_2.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_3.id AS id,",
                    "               row_number() OVER (ORDER BY fruit_3.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_3.name ASC, fruit_3.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_3",
                    "         WHERE color_1.id = fruit_3.color_id",
                    "       ) AS anon_2",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          color_1.id ASC,",
                    "          anon_2.id ASC,",
                    "          color_1.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.id DESC, fruit_2.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_2",
                    "         WHERE fruit_2.color_id IS NOT NULL",
                    "         GROUP BY fruit_2.id,",
                    "                  fruit_2.color_id",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT fruit_3.id AS id,",
                    "               fruit_3.color_id AS color_id,",
                    "               fruit_3.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_3.color_id ORDER BY fruit_3.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_3.color_id ORDER BY fruit_3.name ASC, fruit_3.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_3",
                    "         WHERE fruit_3.color_id IS NOT NULL",
                    "         GROUP BY fruit_3.id,",
                    "                  fruit_3.color_id,",
                    "                  fruit_3.name",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.id AS id_3,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_4,",
                    "       anon_2.rank_1,",
                    "       anon_2.rank_2,",
                    "       anon_1.rank_1 AS rank_1_1,",
                    "       anon_1.rank_2 AS rank_2_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON color_1.id = anon_2.color_id",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          color_1.id ASC,",
                    "          anon_2.id ASC,",
                    "          color_1.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_2.color_id ORDER BY fruit_2.id DESC, fruit_2.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_2",
                    "         WHERE fruit_2.color_id IS NOT NULL",
                    "         GROUP BY fruit_2.id,",
                    "                  fruit_2.color_id",
                    "       ),",
                    "       anon_2 AS (",
                    "        SELECT fruit_3.id AS id,",
                    "               fruit_3.color_id AS color_id,",
                    "               fruit_3.name AS name,",
                    "               dense_rank() OVER (PARTITION BY fruit_3.color_id ORDER BY fruit_3.id ASC) AS rank_1,",
                    "               dense_rank() OVER (PARTITION BY fruit_3.color_id ORDER BY fruit_3.name ASC, fruit_3.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_3",
                    "         WHERE fruit_3.color_id IS NOT NULL",
                    "         GROUP BY fruit_3.id,",
                    "                  fruit_3.color_id,",
                    "                  fruit_3.name",
                    "       ) SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.id AS id_3,",
                    "       fruit_1.name,",
                    "       fruit_1.id AS id_4,",
                    "       anon_2.rank_1,",
                    "       anon_2.rank_2,",
                    "       anon_1.rank_1 AS rank_1_1,",
                    "       anon_1.rank_2 AS rank_2_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN anon_2",
                    "    ON color_1.id = anon_2.color_id",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color_1.id = fruit_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          color_1.id ASC,",
                    "          anon_2.id ASC,",
                    "          color_1.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@pytest.mark.parametrize(
    ("a_order", "b_order"),
    [pytest.param("name: ASC", "name: DESC", id="differing"), pytest.param("name: ASC", "name: ASC", id="equal")],
)
@pytest.mark.allow_duplicate_reads(
    reason="a relation with an ordering of its own, nested under shared aliases, keeps one read per alias"
)
def test_nested_relations_with_arguments_keep_one_read_per_alias(a_order: str, b_order: str) -> None:
    """A relation ordered by its own arguments under each shared alias gets one LATERAL per alias."""
    lines = _shared_sql(
        f"{{ colors {{ id a: fruits(orderBy: {{ sweetness: ASC }}) {{ id color {{ fruits(orderBy: {{ {a_order} }}) "
        f"{{ id }} }} }} b: fruits(orderBy: {{ sweetness: DESC }}) {{ id color {{ fruits(orderBy: {{ {b_order} }}) "
        "{ id } } } } }"
    )

    assert (
        lines
        == snapshot(
            {
                "name: ASC-name: DESC": [
                    "SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.id AS id_3,",
                    "       anon_3.id AS id_4,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.name AS name",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color_1.id = fruit_2.color_id",
                    "         ORDER BY fruit_2.name ASC",
                    "       ) AS anon_2",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_3.id AS id,",
                    "               fruit_3.name AS name",
                    "          FROM fruit AS fruit_3",
                    "         WHERE color_1.id = fruit_3.color_id",
                    "         ORDER BY fruit_3.name DESC",
                    "       ) AS anon_3",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          color_1.id ASC,",
                    "          anon_2.name ASC,",
                    "          color_1.id ASC,",
                    "          anon_3.name DESC",
                ],
                "name: ASC-name: ASC": [
                    "SELECT color.id,",
                    "       anon_1.id AS id_1,",
                    "       color_1.id AS id_2,",
                    "       anon_2.id AS id_3,",
                    "       anon_3.id AS id_4,",
                    "       anon_1.rank_1,",
                    "       anon_1.rank_2",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness ASC, fruit_1.id ASC) AS rank_1,",
                    "               row_number() OVER (ORDER BY fruit_1.sweetness DESC, fruit_1.id ASC) AS rank_2",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = anon_1.color_id",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_2.id AS id,",
                    "               fruit_2.name AS name",
                    "          FROM fruit AS fruit_2",
                    "         WHERE color_1.id = fruit_2.color_id",
                    "         ORDER BY fruit_2.name ASC",
                    "       ) AS anon_2",
                    "    ON TRUE",
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT fruit_3.id AS id,",
                    "               fruit_3.name AS name",
                    "          FROM fruit AS fruit_3",
                    "         WHERE color_1.id = fruit_3.color_id",
                    "         ORDER BY fruit_3.name ASC",
                    "       ) AS anon_3",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          color_1.id ASC,",
                    "          anon_2.name ASC,",
                    "          color_1.id ASC,",
                    "          anon_3.name ASC",
                ],
            }
        )[f"{a_order}-{b_order}"]
    )


def test_hooked_aliases_share_one_lateral() -> None:
    """Aliases of a relation whose hook only filters share one LATERAL, filtered by the hook once."""
    # Pins that the QueryHooks pass builds hook edits as partials, which ``_same_rows`` compares by function and arguments.
    lines = _shared_sql(
        "{ colorsOrderedSweetFruits { id a: fruits(orderBy: { name: ASC }) { id } "
        "b: fruits(orderBy: { name: DESC }) { id } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.id AS id_1,",
            "       anon_1.rank_1,",
            "       anon_1.rank_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.id AS id,",
            "               row_number() OVER (ORDER BY fruit_1.name ASC, fruit_1.id ASC) AS rank_1,",
            "               row_number() OVER (ORDER BY fruit_1.name DESC, fruit_1.id ASC) AS rank_2",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.sweetness > 5",
            "           AND color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ]
    )


def test_aliases_hooked_with_a_limit_stay_separate() -> None:
    """Aliases of a relation whose hook limits its rows keep one LATERAL each, ordered and limited by the hook."""
    lines = _shared_sql(
        "{ colorsOrderedFirstFruits { id a: fruits(orderBy: { name: ASC }) { id } "
        "b: fruits(orderBy: { name: DESC }) { id } } }"
    )

    assert lines == snapshot(
        [
            "SELECT color.id,",
            "       anon_1.id AS id_1,",
            "       anon_2.id AS id_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.id AS id,",
            "               fruit_1.name AS name",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "         ORDER BY fruit_1.name ASC,",
            "                  fruit_1.name ASC",
            "         LIMIT 3",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_2.id AS id,",
            "               fruit_2.name AS name",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "         ORDER BY fruit_2.name ASC,",
            "                  fruit_2.name DESC",
            "         LIMIT 3",
            "       ) AS anon_2",
            "    ON TRUE",
            " ORDER BY color.id ASC,",
            "          anon_1.name ASC,",
            "          anon_1.name ASC,",
            "          anon_2.name ASC,",
            "          anon_2.name DESC",
        ]
    )

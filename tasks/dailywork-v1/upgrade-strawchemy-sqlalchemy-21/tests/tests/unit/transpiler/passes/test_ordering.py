"""Tests for the ``Ordering`` pass: client, default and deterministic ORDER BY of root and relation levels."""

from __future__ import annotations

from importlib import import_module
from typing import TYPE_CHECKING

import pytest
from inline_snapshot import snapshot
from sqlalchemy.dialects import postgresql

from strawchemy.dto.inspectors import SQLAlchemyInspector
from strawchemy.dto.strawberry import GraphQLFieldDefinition, QueryNode
from strawchemy.dto.types import DTOConfig, Purpose
from strawchemy.exceptions import StrawchemyFieldError, TranspilingError
from strawchemy.transpiler._core.level import Level, PlanContext
from strawchemy.transpiler._core.pipeline import Pipeline
from strawchemy.transpiler._core.request import QueryRequest
from strawchemy.transpiler._passes import DEFAULT_PIPELINES
from strawchemy.transpiler._passes.ordering import Ordering
from tests.unit.models import Color, Fruit, Group
from tests.unit.transpiler.passes.utils import plan_sql

if TYPE_CHECKING:
    from collections.abc import Sequence

    from strawchemy.typing import OrderByExpr

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])


def _level(model: type[Color | Fruit], default_order_by: Sequence[OrderByExpr] = ()) -> Level:
    request = QueryRequest(
        model=model,
        selection_tree=None,
        dto_filter=None,
        order_by=(),
        distinct_on=(),
        limit=None,
        offset=None,
        allow_null=False,
    )
    plan_context = PlanContext.create(
        model, postgresql.psycopg2.dialect(), pipelines=DEFAULT_PIPELINES, default_order_by=default_order_by
    )
    return Level.root(request, plan_context)


@_DIALECTS
def test_no_client_order_orders_by_keys(dialect_name: str) -> None:
    """Without client ordering, deterministic ordering orders the root rows by their primary key."""
    lines = plan_sql("{ colors { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": ["SELECT color.name,", "       color.id", "  FROM color AS color", " ORDER BY color.id ASC"],
                "mysql": ["SELECT color.name,", "       color.id", "  FROM color AS color", " ORDER BY color.id ASC"],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_client_order_replaces_keys(dialect_name: str) -> None:
    """A client ordering is used alone: the primary keys are not appended to it."""
    lines = plan_sql("{ colors(orderBy: { name: DESC }) { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_default_order_by_then_keys(dialect_name: str) -> None:
    """Without client ordering, ``default_order_by`` orders the rows, then the primary keys break ties."""
    lines = plan_sql("{ colorsByNameDesc { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC,",
                    "          color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC,",
                    "          color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name DESC,",
                    "          color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_client_order_overrides_default_order_by(dialect_name: str) -> None:
    """A client ordering replaces ``default_order_by``."""
    lines = plan_sql("{ colorsByNameDesc(orderBy: { id: DESC }) { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.id DESC",
                ],
                "sqlite": ["SELECT color.name,", "       color.id", "  FROM color AS color", " ORDER BY color.id DESC"],
                "mysql": ["SELECT color.name,", "       color.id", "  FROM color AS color", " ORDER BY color.id DESC"],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_order_by_to_one_reused_in_wrap(dialect_name: str) -> None:
    """A paginated root ordered by and selecting a to-one relation joins it once, inside the page."""
    lines = plan_sql(
        "{ groupsPaginated(limit: 2, orderBy: { color: { name: ASC } }) { name color { name } } }", dialect_name
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    '       "group".name_1,',
                    '       "group".id_1',
                    "  FROM (",
                    '        SELECT "group".name AS name,',
                    '               "group".id AS id,',
                    "               color_1.name AS name_1,",
                    "               color_1.id AS id_1",
                    '          FROM "group" AS "group"',
                    "          LEFT OUTER JOIN color AS color_1",
                    '            ON color_1.id = "group".color_id',
                    "         ORDER BY color_1.name ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    '       ) AS "group"',
                    ' ORDER BY "group".name_1 ASC,',
                    '          "group".id_1 ASC',
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    '       "group".name_1,',
                    '       "group".id_1',
                    "  FROM (",
                    '        SELECT "group".name AS name,',
                    '               "group".id AS id,',
                    "               color_1.name AS name_1,",
                    "               color_1.id AS id_1",
                    '          FROM "group" AS "group"',
                    "          LEFT OUTER JOIN color AS color_1",
                    '            ON color_1.id = "group".color_id',
                    "         ORDER BY color_1.name ASC",
                    "         LIMIT ?",
                    "        OFFSET ?",
                    '       ) AS "group"',
                    ' ORDER BY "group".name_1 ASC,',
                    '          "group".id_1 ASC',
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id,",
                    "       `group`.name_1,",
                    "       `group`.id_1",
                    "  FROM (",
                    "        SELECT `group`.name AS name,",
                    "               `group`.id AS id,",
                    "               color_1.name AS name_1,",
                    "               color_1.id AS id_1",
                    "          FROM `group` AS `group`",
                    "          LEFT OUTER JOIN color AS color_1",
                    "            ON color_1.id = `group`.color_id",
                    "         ORDER BY color_1.name ASC",
                    "         LIMIT %s,",
                    "               %s",
                    "       ) AS `group`",
                    " ORDER BY `group`.name_1 ASC,",
                    "          `group`.id_1 ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_relation_own_order_without_keys(dialect_name: str) -> None:
    """A relation ordered by the client orders its own rows by that ordering only, after the root's keys."""
    lines = plan_sql("{ colors { fruits(orderBy: { name: DESC }) { name } } }", dialect_name)

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
                    "         ORDER BY fruit_1.name DESC",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name DESC",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.name DESC, fruit_1.id) AS rank",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.name DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name DESC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT fruit_1.name AS name,",
                    "               fruit_1.id AS id,",
                    "               fruit_1.color_id AS color_id,",
                    "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY fruit_1.name DESC, fruit_1.id) AS `rank`",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.name,",
                    "                  fruit_1.id,",
                    "                  fruit_1.color_id",
                    "         ORDER BY fruit_1.name DESC",
                    "       ) SELECT color.id,",
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color.id = anon_1.color_id",
                    " ORDER BY color.id ASC,",
                    "          anon_1.name DESC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_paginated_relation_orders_page_by_keys(dialect_name: str) -> None:
    """A paginated relation without client ordering orders its page by its primary key, inside its LATERAL or CTE."""
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


def test_default_order_by_invalid_column_raises() -> None:
    """A ``default_order_by`` on a column of another model fails, at schema build and when ordering a level."""
    with pytest.raises(StrawchemyFieldError, match="not a column"):
        import_module("tests.unit.schemas.default_order_by_invalid")

    level = _level(Fruit, default_order_by=[Group.tag_id.asc()])
    with pytest.raises(StrawchemyFieldError, match="`default_order_by` column 'tag_id' is not a column of Fruit"):
        Pipeline((Ordering(),)).plan(level)


def test_order_by_leaf_without_direction_raises() -> None:
    """An order-by leaf carrying no direction is rejected."""
    level = _level(Color)
    inspector = SQLAlchemyInspector([Color.registry])
    field = inspector.field_definition(Color.__mapper__.attrs["name"].class_attribute, DTOConfig(Purpose.READ))
    level.request.__dict__["order_by_nodes"] = (QueryNode(value=GraphQLFieldDefinition.from_field(field)),)

    with pytest.raises(TranspilingError, match="Missing order by value"):
        Pipeline((Ordering(),)).plan(level)

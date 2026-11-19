"""Tests for the ``QueryHooks`` pass: hook edits on a level's rows and hook loads in its projection."""

from __future__ import annotations

import pytest
from inline_snapshot import snapshot

from tests.unit.transpiler.passes.utils import plan_sql

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])

_FILTERED_ON_COLOR = '(filter: { color: { name: { eq: "red" } } })'


@_DIALECTS
def test_hook_where_on_relation_in_on_clause(dialect_name: str) -> None:
    """A to-many relation whose hook adds only a WHERE is a plain LEFT OUTER JOIN carrying it in its ON clause."""
    lines = plan_sql("{ colorsSweetFruits { name fruits { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "   AND fruit_1.sweetness > %(sweetness_1)s",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "   AND fruit_1.sweetness > ?",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM color AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    "   AND fruit_1.sweetness > %s",
                    " ORDER BY color.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_hooked_to_one_filtered_and_selected_joins_twice(dialect_name: str) -> None:
    """A to-one relation filtered and selected through a WHERE hook is joined twice, the selection with the hook."""
    lines = plan_sql(f"{{ groupsVisibleColor{_FILTERED_ON_COLOR} {{ name color {{ name }} }} }}", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_2",
                    '    ON color_2.id = "group".color_id',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != %(name_2)s",
                    " WHERE color_2.name = %(name_3)s",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_2",
                    '    ON color_2.id = "group".color_id',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != ?",
                    " WHERE color_2.name = ?",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id,",
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM `group` AS `group`",
                    " INNER JOIN color AS color_2",
                    "    ON color_2.id = `group`.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    "   AND color_1.name != %s",
                    " WHERE color_2.name = %s",
                    " ORDER BY `group`.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_filter_on_hooked_relation_no_subquery(dialect_name: str) -> None:
    """Without pagination, a filter on a hooked to-one relation joins it in the query itself, with no subquery."""
    lines = plan_sql(f"{{ groupsVisibleColor{_FILTERED_ON_COLOR} {{ name color {{ name }} }} }}", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_2",
                    '    ON color_2.id = "group".color_id',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != %(name_2)s",
                    " WHERE color_2.name = %(name_3)s",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  JOIN color AS color_2",
                    '    ON color_2.id = "group".color_id',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != ?",
                    " WHERE color_2.name = ?",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id,",
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM `group` AS `group`",
                    " INNER JOIN color AS color_2",
                    "    ON color_2.id = `group`.color_id",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    "   AND color_1.name != %s",
                    " WHERE color_2.name = %s",
                    " ORDER BY `group`.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_filter_on_hooked_relation_paginated_joins_hook_outside_page(dialect_name: str) -> None:
    """With pagination, the filter join runs inside the page and the hooked selection join outside it."""
    lines = plan_sql(
        '{ groupsVisibleColorPaginated(limit: 2, filter: { color: { name: { eq: "red" } } }) { name color { name } } }',
        dialect_name,
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM (",
                    '        SELECT "group".name AS name,',
                    '               "group".id AS id,',
                    '               "group".color_id AS color_id',
                    '          FROM "group" AS "group"',
                    "          JOIN color AS color_2",
                    '            ON color_2.id = "group".color_id',
                    "         WHERE color_2.name = %(name_2)s",
                    '         ORDER BY "group".id ASC',
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    '       ) AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != %(name_3)s",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM (",
                    '        SELECT "group".name AS name,',
                    '               "group".id AS id,',
                    '               "group".color_id AS color_id',
                    '          FROM "group" AS "group"',
                    "          JOIN color AS color_2",
                    '            ON color_2.id = "group".color_id',
                    "         WHERE color_2.name = ?",
                    '         ORDER BY "group".id ASC',
                    "         LIMIT ?",
                    "        OFFSET ?",
                    '       ) AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "   AND color_1.name != ?",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id,",
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM (",
                    "        SELECT `group`.name AS name,",
                    "               `group`.id AS id,",
                    "               `group`.color_id AS color_id",
                    "          FROM `group` AS `group`",
                    "         INNER JOIN color AS color_2",
                    "            ON color_2.id = `group`.color_id",
                    "         WHERE color_2.name = %s",
                    "         ORDER BY `group`.id ASC",
                    "         LIMIT %s,",
                    "               %s",
                    "       ) AS `group`",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    "   AND color_1.name != %s",
                    " ORDER BY `group`.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_root_hook_join_with_pagination(dialect_name: str) -> None:
    """A root hook adding a JOIN runs inside the page, which exports the columns the hook loads."""
    lines = plan_sql("{ coloredFruitsPaginated(limit: 2) { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM (",
                    "        SELECT fruit.name AS name,",
                    "               fruit.id AS id,",
                    "               fruit.sweetness AS sweetness",
                    "          FROM fruit AS fruit",
                    "          JOIN color AS hook_color",
                    "            ON hook_color.id = fruit.color_id",
                    "         WHERE hook_color.name != %(name_1)s",
                    "         ORDER BY fruit.id ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    "       ) AS fruit",
                    " ORDER BY fruit.id ASC",
                ],
                "sqlite": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM (",
                    "        SELECT fruit.name AS name,",
                    "               fruit.id AS id,",
                    "               fruit.sweetness AS sweetness",
                    "          FROM fruit AS fruit",
                    "          JOIN color AS hook_color",
                    "            ON hook_color.id = fruit.color_id",
                    "         WHERE hook_color.name != ?",
                    "         ORDER BY fruit.id ASC",
                    "         LIMIT ?",
                    "        OFFSET ?",
                    "       ) AS fruit",
                    " ORDER BY fruit.id ASC",
                ],
                "mysql": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM (",
                    "        SELECT fruit.name AS name,",
                    "               fruit.id AS id,",
                    "               fruit.sweetness AS sweetness",
                    "          FROM fruit AS fruit",
                    "         INNER JOIN color AS hook_color",
                    "            ON hook_color.id = fruit.color_id",
                    "         WHERE hook_color.name != %s",
                    "         ORDER BY fruit.id ASC",
                    "         LIMIT %s,",
                    "               %s",
                    "       ) AS fruit",
                    " ORDER BY fruit.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_hook_order_only_to_one_reuses_row_join(dialect_name: str) -> None:
    """A to-one relation ordered on and selected, whose hook adds only an ORDER BY, is joined once."""
    lines = plan_sql(
        "{ groupsNameOrderedColor(orderBy: { color: { name: ASC } }) { name color { name } } }", dialect_name
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " ORDER BY color_1.name ASC,",
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    'SELECT "group".name,',
                    '       "group".id,',
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    " ORDER BY color_1.name ASC,",
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "SELECT `group`.name,",
                    "       `group`.id,",
                    "       color_1.name AS name_1,",
                    "       color_1.id AS id_1",
                    "  FROM `group` AS `group`",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    " ORDER BY color_1.name ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_root_hook_order_only_orders_inline(dialect_name: str) -> None:
    """A root hook adding only an ORDER BY orders the root rows in the query itself, with no subquery."""
    lines = plan_sql("{ nameOrderedColors { id name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM color AS color",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_root_hook_join_without_pagination_joins_inline(dialect_name: str) -> None:
    """Without pagination, a root hook adding a JOIN joins in the query itself, with no subquery."""
    lines = plan_sql("{ coloredFruits { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM fruit AS fruit",
                    "  JOIN color AS hook_color",
                    "    ON hook_color.id = fruit.color_id",
                    " WHERE hook_color.name != %(name_1)s",
                    " ORDER BY fruit.id ASC",
                ],
                "sqlite": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM fruit AS fruit",
                    "  JOIN color AS hook_color",
                    "    ON hook_color.id = fruit.color_id",
                    " WHERE hook_color.name != ?",
                    " ORDER BY fruit.id ASC",
                ],
                "mysql": [
                    "SELECT fruit.name,",
                    "       fruit.sweetness,",
                    "       fruit.id",
                    "  FROM fruit AS fruit",
                    " INNER JOIN color AS hook_color",
                    "    ON hook_color.id = fruit.color_id",
                    " WHERE hook_color.name != %s",
                    " ORDER BY fruit.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_root_hook_limit_runs_in_a_page_before_to_many_joins(dialect_name: str) -> None:
    """A root hook adding a LIMIT limits the root rows in a page subquery, and the to-many join runs outside it."""
    lines = plan_sql("{ firstColors { name fruits { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.name ASC,",
                    "                  color.id ASC",
                    "         LIMIT %(param_1)s",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.name ASC,",
                    "                  color.id ASC",
                    "         LIMIT ?",
                    "        OFFSET ?",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.name ASC,",
                    "                  color.id ASC",
                    "         LIMIT %s",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          color.id ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )

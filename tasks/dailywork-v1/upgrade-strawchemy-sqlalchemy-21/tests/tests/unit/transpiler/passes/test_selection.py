"""Tests for the ``Selection`` pass: loaded columns and identity columns of one level."""

from __future__ import annotations

import pytest
from inline_snapshot import snapshot

from tests.unit.transpiler.passes.utils import plan_sql

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])


@_DIALECTS
def test_selection_loads_only_selected_columns(dialect_name: str) -> None:
    """A selection of one column loads that column and the primary key, and no other column."""
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
def test_selection_loads_primary_key_without_columns(dialect_name: str) -> None:
    """A level selecting only a relation still loads its primary key, not its other columns."""
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
def test_selection_selects_identity_of_relation_owning_computed_values(dialect_name: str) -> None:
    """A relation owning a computed value selects its primary key once more, for the executor to key the value on."""
    lines = plan_sql("{ groups { color { fruitsAggregate { count } } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    'SELECT "group".id,',
                    "       color_1.id AS id_1,",
                    "       color_1.id AS group__color__id,",
                    "       anon_1.count_1",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "  JOIN LATERAL (",
                    "        SELECT count(*) AS count_1",
                    "          FROM fruit AS fruit_1",
                    "         WHERE color_1.id = fruit_1.color_id",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "sqlite": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    '       ) SELECT "group".id,',
                    "       color_1.id AS id_1,",
                    "       color_1.id AS group__color__id,",
                    "       coalesce(anon_1.count_1, ?) AS coalesce_1",
                    '  FROM "group" AS "group"',
                    "  LEFT OUTER JOIN color AS color_1",
                    '    ON color_1.id = "group".color_id',
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color_1.id = anon_1.color_id",
                    ' ORDER BY "group".id ASC,',
                    "          color_1.id ASC",
                ],
                "mysql": [
                    "WITH anon_1 AS (",
                    "        SELECT count(*) AS count_1,",
                    "               fruit_1.color_id AS color_id",
                    "          FROM fruit AS fruit_1",
                    "         WHERE fruit_1.color_id IS NOT NULL",
                    "         GROUP BY fruit_1.color_id",
                    "       ) SELECT `group`.id,",
                    "       color_1.id AS id_1,",
                    "       color_1.id AS group__color__id,",
                    "       coalesce(anon_1.count_1, %s) AS coalesce_1",
                    "  FROM `group` AS `group`",
                    "  LEFT OUTER JOIN color AS color_1",
                    "    ON color_1.id = `group`.color_id",
                    "  LEFT OUTER JOIN anon_1",
                    "    ON color_1.id = anon_1.color_id",
                    " ORDER BY `group`.id ASC,",
                    "          color_1.id ASC",
                ],
            }
        )[dialect_name]
    )

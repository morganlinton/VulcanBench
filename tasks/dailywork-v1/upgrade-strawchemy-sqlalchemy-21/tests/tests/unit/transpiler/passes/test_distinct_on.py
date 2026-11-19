"""Tests for the ``DistinctOn`` pass: native or emulated DISTINCT ON, and the page it runs in."""

from __future__ import annotations

import pytest
from inline_snapshot import snapshot

from tests.unit.transpiler.passes.utils import plan_sql

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])


@_DIALECTS
def test_distinct_native_vs_emulated(dialect_name: str) -> None:
    """DISTINCT ON is native on postgresql when the ORDER BY starts with its columns, a ``row_number`` rank elsewhere."""
    lines = plan_sql("{ colorsDistinct(distinctOn: [name], orderBy: { name: ASC }) { name } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM (",
                    "        SELECT DISTINCT",
                    "            ON (color.name) color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.name ASC",
                    "       ) AS color",
                    " ORDER BY color.name ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM (",
                    "        SELECT anon_1.name AS name,",
                    "               anon_1.id AS id",
                    "          FROM (",
                    "                SELECT color.name AS name,",
                    "                       color.id AS id,",
                    "                       row_number() OVER (PARTITION BY color.name ORDER BY color.name ASC) AS anon_2",
                    "                  FROM color AS color",
                    "               ) AS anon_1",
                    "         WHERE anon_1.anon_2 = ?",
                    "         ORDER BY anon_1.name ASC",
                    "       ) AS color",
                    " ORDER BY color.name ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id",
                    "  FROM (",
                    "        SELECT anon_1.name AS name,",
                    "               anon_1.id AS id",
                    "          FROM (",
                    "                SELECT color.name AS name,",
                    "                       color.id AS id,",
                    "                       row_number() OVER (PARTITION BY color.name ORDER BY color.name ASC) AS anon_2",
                    "                  FROM color AS color",
                    "               ) AS anon_1",
                    "         WHERE anon_1.anon_2 = %s",
                    "         ORDER BY anon_1.name ASC",
                    "       ) AS color",
                    " ORDER BY color.name ASC",
                ],
            }
        )[dialect_name]
    )


def test_distinct_without_order_prefix_is_emulated() -> None:
    """On postgresql, an ORDER BY not starting with the DISTINCT ON columns makes DISTINCT ON a ``row_number`` rank."""
    lines = plan_sql("{ colorsDistinct(distinctOn: [name], orderBy: { id: ASC }) { name } }", "postgresql")

    assert lines == snapshot(
        [
            "SELECT color.name,",
            "       color.id",
            "  FROM (",
            "        SELECT anon_1.name AS name,",
            "               anon_1.id AS id",
            "          FROM (",
            "                SELECT color.name AS name,",
            "                       color.id AS id,",
            "                       row_number() OVER (PARTITION BY color.name ORDER BY color.id ASC) AS anon_2",
            "                  FROM color AS color",
            "               ) AS anon_1",
            "         WHERE anon_1.anon_2 = %(param_1)s",
            "         ORDER BY anon_1.id ASC",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ]
    )


def test_distinct_on_more_columns_than_order_by_is_emulated() -> None:
    """On postgresql, an ORDER BY shorter than the DISTINCT ON columns makes DISTINCT ON a ``row_number`` rank."""
    lines = plan_sql("{ colorsDistinct(distinctOn: [name, id], orderBy: { name: ASC }) { name } }", "postgresql")

    assert lines == snapshot(
        [
            "SELECT color.name,",
            "       color.id",
            "  FROM (",
            "        SELECT anon_1.name AS name,",
            "               anon_1.id AS id",
            "          FROM (",
            "                SELECT color.name AS name,",
            "                       color.id AS id,",
            "                       row_number() OVER (PARTITION BY color.name, color.id ORDER BY color.name ASC) AS anon_2",
            "                  FROM color AS color",
            "               ) AS anon_1",
            "         WHERE anon_1.anon_2 = %(param_1)s",
            "         ORDER BY anon_1.name ASC",
            "       ) AS color",
            " ORDER BY color.name ASC",
        ]
    )


@_DIALECTS
def test_distinct_selecting_relation_wraps(dialect_name: str) -> None:
    """DISTINCT ON next to a selected to-many relation runs in the page, the relation is joined outside it."""
    lines = plan_sql(
        "{ colorsDistinct(distinctOn: [name], orderBy: { name: ASC }) { name fruits { name } } }", dialect_name
    )

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
                    "        SELECT DISTINCT",
                    "            ON (color.name) color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.name ASC",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          fruit_1.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM (",
                    "        SELECT anon_1.name AS name,",
                    "               anon_1.id AS id",
                    "          FROM (",
                    "                SELECT color.name AS name,",
                    "                       color.id AS id,",
                    "                       row_number() OVER (PARTITION BY color.name ORDER BY color.name ASC) AS anon_2",
                    "                  FROM color AS color",
                    "               ) AS anon_1",
                    "         WHERE anon_1.anon_2 = ?",
                    "         ORDER BY anon_1.name ASC",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          fruit_1.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       fruit_1.name AS name_1,",
                    "       fruit_1.id AS id_1",
                    "  FROM (",
                    "        SELECT anon_1.name AS name,",
                    "               anon_1.id AS id",
                    "          FROM (",
                    "                SELECT color.name AS name,",
                    "                       color.id AS id,",
                    "                       row_number() OVER (PARTITION BY color.name ORDER BY color.name ASC) AS anon_2",
                    "                  FROM color AS color",
                    "               ) AS anon_1",
                    "         WHERE anon_1.anon_2 = %s",
                    "         ORDER BY anon_1.name ASC",
                    "       ) AS color",
                    "  LEFT OUTER JOIN fruit AS fruit_1",
                    "    ON color.id = fruit_1.color_id",
                    " ORDER BY color.name ASC,",
                    "          fruit_1.id ASC",
                ],
            }
        )[dialect_name]
    )

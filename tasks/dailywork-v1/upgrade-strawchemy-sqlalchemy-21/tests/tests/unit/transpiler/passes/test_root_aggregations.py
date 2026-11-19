"""Tests for the ``RootAggregations`` pass: window functions over the root rows."""

from __future__ import annotations

import pytest
from inline_snapshot import snapshot

from tests.unit.transpiler.passes.utils import plan_sql

_DIALECTS = pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])


@_DIALECTS
def test_root_aggregation_over_page(dialect_name: str) -> None:
    """A root aggregation of paginated rows is a window function of the outer SELECT, computed over the page."""
    lines = plan_sql("{ colorAggregationsPaginated(limit: 2) { aggregations { count } nodes { name } } }", dialect_name)

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       count(*) OVER () AS anon_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       count(*) OVER () AS anon_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT ?",
                    "        OFFSET ?",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.name,",
                    "       color.id,",
                    "       count(*) OVER () AS anon_1",
                    "  FROM (",
                    "        SELECT color.name AS name,",
                    "               color.id AS id",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT %s,",
                    "               %s",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )


@_DIALECTS
def test_root_aggregation_reads_root_alias(dialect_name: str) -> None:
    """Without pagination, the window functions read the root alias, one per selected argument."""
    lines = plan_sql(
        "{ colorAggregationsPaginated { aggregations { count max { name } } nodes { id } } }", dialect_name
    )

    assert (
        lines
        == snapshot(
            {
                "postgresql": [
                    "SELECT color.id,",
                    "       count(*) OVER () AS anon_1,",
                    "       max(color.name) OVER () AS anon_2",
                    "  FROM (",
                    "        SELECT color.id AS id,",
                    "               color.name AS name",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
                "sqlite": [
                    "SELECT color.id,",
                    "       count(*) OVER () AS anon_1,",
                    "       max(color.name) OVER () AS anon_2",
                    "  FROM (",
                    "        SELECT color.id AS id,",
                    "               color.name AS name",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT ?",
                    "        OFFSET ?",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
                "mysql": [
                    "SELECT color.id,",
                    "       count(*) OVER () AS anon_1,",
                    "       max(color.name) OVER () AS anon_2",
                    "  FROM (",
                    "        SELECT color.id AS id,",
                    "               color.name AS name",
                    "          FROM color AS color",
                    "         ORDER BY color.id ASC",
                    "         LIMIT %s,",
                    "               %s",
                    "       ) AS color",
                    " ORDER BY color.id ASC",
                ],
            }
        )[dialect_name]
    )

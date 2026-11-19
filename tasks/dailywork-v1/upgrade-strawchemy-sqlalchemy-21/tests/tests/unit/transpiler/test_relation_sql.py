"""SQL of relations planned on their own: secondary-table LATERAL joins and nested sort keys."""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING, Any

import pytest
from inline_snapshot import snapshot
from sqlalchemy.dialects import postgresql
from sqlalchemy.exc import SAWarning
from sqlalchemy.sql.compiler import FROM_LINTING

from tests.unit.schemas.optimizations import schema as optimizations_schema
from tests.unit.schemas.secondary_table import schema as secondary_table_schema
from tests.unit.utils import SQLA_DIALECTS, DialectContext
from tests.utils import format_sql

if TYPE_CHECKING:
    from sqlalchemy import Select


def _linted_sql(statement: Select[Any]) -> str:
    """Compiles for postgres with the FROM linter on, so a cartesian product raises.

    Args:
        statement: The statement to compile.

    Raises:
        SAWarning: If the compiled statement has unrelated FROM elements.
    """
    with warnings.catch_warnings():
        warnings.simplefilter("error", category=SAWarning)
        return format_sql(str(statement.compile(dialect=postgresql.psycopg2.dialect(), linting=FROM_LINTING)))


@pytest.mark.parametrize(
    ("query", "expected"),
    [
        pytest.param(
            "{ users { id departmentsAggregate { count } } }",
            snapshot(
                [
                    'SELECT "user".id,',
                    "       anon_1.count_1",
                    '  FROM "user" AS "user"',
                    "  JOIN LATERAL (",
                    "        SELECT count(*) AS count_1",
                    "          FROM department AS department_1",
                    "          JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    '         WHERE "user".id = user_department_join_table_1.user_id',
                    "       ) AS anon_1",
                    "    ON TRUE",
                    ' ORDER BY "user".id ASC',
                ]
            ),
            id="aggregation",
        ),
        pytest.param(
            "{ users { id departments(limit: 2) { id name } } }",
            snapshot(
                [
                    'SELECT "user".id,',
                    "       anon_1.name,",
                    "       anon_1.id AS id_1",
                    '  FROM "user" AS "user"',
                    "  LEFT OUTER JOIN LATERAL (",
                    "        SELECT department_1.id AS id,",
                    "               department_1.name AS name",
                    "          FROM department AS department_1",
                    "          JOIN user_department_join_table AS user_department_join_table_1",
                    "            ON department_1.id = user_department_join_table_1.department_id",
                    '         WHERE "user".id = user_department_join_table_1.user_id',
                    "         ORDER BY department_1.id ASC",
                    "         LIMIT %(param_1)s",
                    "        OFFSET %(param_2)s",
                    "       ) AS anon_1",
                    "    ON TRUE",
                    ' ORDER BY "user".id ASC,',
                    "          anon_1.id ASC",
                ]
            ),
            id="relation",
        ),
    ],
)
@pytest.mark.inline_snapshot
def test_secondary_lateral_joins_the_secondary_table(
    query: str, expected: list[str], captured_statements: list[Select[Any]]
) -> None:
    """A secondary-table lateral joins the secondary table instead of correlating it in WHERE."""
    result = secondary_table_schema.execute_sync(query, context_value=DialectContext("postgresql"))

    assert not result.errors
    assert _linted_sql(captured_statements[0]).splitlines() == expected


RELATION_ORDER_BY_SQL = snapshot(
    {
        "postgresql": [
            "SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN LATERAL (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.id AS id,",
            "               color_1.name AS name_1,",
            "               anon_2.count_1 AS count_1,",
            "               fruit_1.sweetness AS sweetness",
            "          FROM fruit AS fruit_1",
            "          LEFT OUTER JOIN color AS color_1",
            "            ON color_1.id = fruit_1.color_id",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1",
            "                  FROM fruit AS fruit_2",
            "                 WHERE color_1.id = fruit_2.color_id",
            "               ) AS anon_2",
            "            ON TRUE",
            "         WHERE color.id = fruit_1.color_id",
            "         ORDER BY color_1.name ASC,",
            "                  anon_2.count_1 DESC,",
            "                  fruit_1.sweetness ASC",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC,",
            "          anon_1.name_1 ASC,",
            "          anon_1.count_1 DESC,",
            "          anon_1.sweetness ASC",
        ],
        "mysql": [
            "WITH anon_2 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ),",
            "       anon_1 AS (",
            "        SELECT fruit_1.name AS name,",
            "               fruit_1.id AS id,",
            "               fruit_1.color_id AS color_id,",
            "               color_1.name AS name_1,",
            "               coalesce(anon_2.count_1, %s) AS coalesce_1,",
            "               fruit_1.sweetness AS sweetness,",
            "               dense_rank() OVER (PARTITION BY fruit_1.color_id ORDER BY color_1.name ASC, coalesce(anon_2.count_1, %s) DESC, fruit_1.sweetness ASC, fruit_1.id) AS `rank`",
            "          FROM fruit AS fruit_1",
            "          LEFT OUTER JOIN color AS color_1",
            "            ON color_1.id = fruit_1.color_id",
            "          LEFT OUTER JOIN anon_2",
            "            ON color_1.id = anon_2.color_id",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.name,",
            "                  fruit_1.id,",
            "                  fruit_1.color_id,",
            "                  color_1.name,",
            "                  coalesce(anon_2.count_1, %s),",
            "                  fruit_1.sweetness",
            "         ORDER BY color_1.name ASC,",
            "                  coalesce(anon_2.count_1, %s) DESC, fruit_1.sweetness ASC",
            "       ) SELECT color.id,",
            "       anon_1.name,",
            "       anon_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY color.id ASC,",
            "          anon_1.name_1 ASC,",
            "          anon_1.coalesce_1 DESC,",
            "          anon_1.sweetness ASC",
        ],
    }
)


@pytest.mark.parametrize("dialect_name", ["postgresql", "mysql"])
@pytest.mark.inline_snapshot
def test_relation_order_by_exposes_nested_sort_keys(dialect_name: str, captured_statements: list[Select[Any]]) -> None:
    """The outer ORDER BY reads each sort key of a relation's own ordering from a column its subquery exposes."""
    result = optimizations_schema.execute_sync(
        """{
            colors {
                id
                fruits(
                    orderBy: [
                        { color: { name: ASC } }
                        { color: { fruitsAggregate: { count: DESC } } }
                        { sweetness: ASC }
                    ]
                ) {
                    name
                }
            }
        }""",
        context_value=DialectContext(dialect_name),  # ty: ignore[invalid-argument-type]
    )

    assert not result.errors
    compiled = str(captured_statements[0].compile(dialect=SQLA_DIALECTS[dialect_name]))
    assert format_sql(compiled).splitlines() == RELATION_ORDER_BY_SQL[dialect_name]

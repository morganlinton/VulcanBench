"""Hidden test for oss-sqlglot-qualify-lateral-star.

The optimizer's qualify step must expand SELECT * to include the columns produced by a
CROSS JOIN LATERAL subquery, and must handle a partial column-alias list (fewer aliases
than the subquery projects). Expected outputs captured from the upstream fix (sqlglot
PR #7802).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"t": {"k": "INT"}}


def _q(sql: str) -> str:
    return qualify(sqlglot.parse_one(sql), schema=SCHEMA).sql()


def test_select_star_expands_lateral_columns():
    got = _q("SELECT * FROM t CROSS JOIN LATERAL (SELECT 1 AS x, 2 AS y) AS foo")
    assert got == (
        'SELECT "t"."k" AS "k", "foo"."x" AS "x", "foo"."y" AS "y" '
        'FROM "t" AS "t" CROSS JOIN LATERAL (SELECT 1 AS "x", 2 AS "y") AS "foo"'
    )


def test_select_star_partial_column_alias():
    got = _q("SELECT * FROM t CROSS JOIN LATERAL (SELECT 1 AS a, 2 AS b) AS x(c)")
    assert got == (
        'SELECT "t"."k" AS "k", "x"."c" AS "c", "x"."b" AS "b" '
        'FROM "t" AS "t" CROSS JOIN LATERAL (SELECT 1 AS "a", 2 AS "b") AS "x"("c")'
    )


def test_non_lateral_star_unaffected():
    # pass_to_pass no-regression anchor.
    assert _q("SELECT * FROM t") == 'SELECT "t"."k" AS "k" FROM "t" AS "t"'

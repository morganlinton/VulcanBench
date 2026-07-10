"""Hidden tests for oss-sqlglot-qualify-lateral-star.

The optimizer's qualify step must expand SELECT * to include the columns produced by a
CROSS JOIN LATERAL subquery, must expand a qualified star (foo.*) over a lateral, and
must handle a partial column-alias list (fewer aliases than the subquery projects).
At the unfixed base, a star over a lateral is left unexpanded.
Expected outputs captured from the upstream fix (sqlglot PR #7802).
"""
import sqlglot
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"t": {"k": "INT"}}


def _q(sql: str) -> str:
    return qualify(sqlglot.parse_one(sql), schema=SCHEMA).sql()


# --- fail_to_pass ---

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


def test_select_star_expands_single_column_lateral():
    got = _q("SELECT * FROM t CROSS JOIN LATERAL (SELECT 1 AS z) AS y")
    assert got == (
        'SELECT "t"."k" AS "k", "y"."z" AS "z" '
        'FROM "t" AS "t" CROSS JOIN LATERAL (SELECT 1 AS "z") AS "y"'
    )


def test_qualified_star_expands_lateral_columns():
    got = _q("SELECT foo.* FROM t CROSS JOIN LATERAL (SELECT 1 AS x, 2 AS y) AS foo")
    assert got == (
        'SELECT "foo"."x" AS "x", "foo"."y" AS "y" '
        'FROM "t" AS "t" CROSS JOIN LATERAL (SELECT 1 AS "x", 2 AS "y") AS "foo"'
    )


# --- pass_to_pass (unchanged by the fix) ---

def test_non_lateral_star_unaffected():
    assert _q("SELECT * FROM t") == 'SELECT "t"."k" AS "k" FROM "t" AS "t"'


def test_full_column_alias_list_unaffected():
    # A complete alias list already expanded correctly at base.
    got = _q("SELECT * FROM t CROSS JOIN LATERAL (SELECT 1 AS a, 2 AS b) AS x(c, d)")
    assert got == (
        'SELECT "t"."k" AS "k", "x"."c" AS "c", "x"."d" AS "d" '
        'FROM "t" AS "t" CROSS JOIN LATERAL (SELECT 1 AS "a", 2 AS "b") AS "x"("c", "d")'
    )

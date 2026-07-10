"""Hidden test for oss-sqlglot-canonicalize-internal-names.

A new optimizer pass ``canonicalize_internal_names`` must rewrite a qualified
query to a canonical structural form: base-table names/columns and top-level
output aliases are preserved (the data contract), while internal names (table
aliases, CTE and subquery names, internal column aliases) become sequential
``_tN`` / ``_cN``. Graded on exact-output parity across several independent
query shapes plus contract-difference properties. Expected behavior captured
from the upstream feature (tobymao/sqlglot PR #7580).

Run with PYTHONPATH=. so the workspace sqlglot is under test.
"""
from __future__ import annotations

import sqlglot
from sqlglot import parse_one
from sqlglot.optimizer.qualify import qualify

SCHEMA = {"x": {"a": "INT", "b": "INT"}, "y": {"b": "INT", "c": "INT"}}


def canon(sql: str, schema=None, catalog="c", db="db") -> str:
    from sqlglot.optimizer.canonicalize_internal_names import canonicalize_internal_names

    q = qualify(parse_one(sql), schema=schema or SCHEMA, catalog=catalog, db=db)
    return canonicalize_internal_names(q).sql()


def test_simple_select_and_star_expansion():
    assert canon("SELECT a FROM x") == 'SELECT "_t0"."a" AS "a" FROM "c"."db"."x" AS "_t0"'


def test_cte_internal_names_canonicalized():
    assert canon("WITH t AS (SELECT a, b FROM x) SELECT * FROM t") == (
        'WITH "_t1" AS (SELECT "_t0"."a" AS "_c0", "_t0"."b" AS "_c1" '
        'FROM "c"."db"."x" AS "_t0") SELECT "_t1"."_c0" AS "a", "_t1"."_c1" AS "b" '
        'FROM "_t1" AS "_t1"'
    )


def test_subquery_column_aliases_are_internal_handles():
    assert canon("SELECT t.p, t.q FROM (SELECT a, b FROM x) AS t(p, q)") == (
        'SELECT "_t1"."_c0" AS "p", "_t1"."_c1" AS "q" FROM '
        '(SELECT "_t0"."a" AS "_c0", "_t0"."b" AS "_c1" FROM "c"."db"."x" AS "_t0") AS "_t1"'
    )


def test_correlated_subquery_traversal_order():
    assert canon("SELECT a FROM x WHERE EXISTS (SELECT 1 FROM y WHERE y.b = x.b)") == (
        'SELECT "_t1"."a" AS "a" FROM "c"."db"."x" AS "_t1" WHERE '
        'EXISTS(SELECT 1 AS "_c0" FROM "c"."db"."y" AS "_t0" WHERE "_t0"."b" = "_t1"."b")'
    )


def test_union_left_branch_is_data_contract():
    assert canon("SELECT a FROM x UNION SELECT b FROM y") == (
        'SELECT "_t0"."a" AS "a" FROM "c"."db"."x" AS "_t0" UNION '
        'SELECT "_t1"."b" AS "_c0" FROM "c"."db"."y" AS "_t1"'
    )


def test_contract_differences_stay_distinct():
    # Same shape, different base table or column names -> different canonical form.
    a = canon(
        "SELECT id, name FROM cat.db.users WHERE id > 5",
        schema={"cat": {"db": {"users": {"id": "INT", "name": "TEXT"}}}},
    )
    b = canon(
        "SELECT id, name FROM cat.db.employees WHERE id > 5",
        schema={"cat": {"db": {"employees": {"id": "INT", "name": "TEXT"}}}},
    )
    assert a != b


def test_qualify_alone_unchanged():
    # The ordinary qualify pipeline (no canonicalization) must keep working.
    q = qualify(parse_one("SELECT a FROM x"), schema=SCHEMA, catalog="c", db="db")
    assert q.sql() == 'SELECT "x"."a" AS "a" FROM "c"."db"."x" AS "x"'

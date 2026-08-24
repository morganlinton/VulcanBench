"""Hidden pass-to-pass guards: legitimate column pruning must keep working.

A fix that simply disables projection pushdown passes the fail-to-pass tests
but fails these — the guards pin cases where pruning is safe and required.
"""

from __future__ import annotations

from sqlglot import exp, parse_one
from sqlglot.optimizer import optimize

SCHEMA = {
    "x": {"a": "INT", "b": "INT", "c": "INT"},
    "y": {"a": "INT", "b": "INT", "c": "INT"},
}


def optimized_sql(sql: str, dialect: str = "duckdb") -> str:
    return optimize(parse_one(sql, read=dialect), schema=SCHEMA, dialect=dialect).sql(dialect=dialect)


def optimized(sql: str, dialect: str = "duckdb") -> exp.Expression:
    return parse_one(optimized_sql(sql, dialect), read=dialect)


def test_unused_columns_are_still_pruned_from_plain_subqueries():
    tree = optimized("SELECT t.a FROM (SELECT a, b, c FROM x) t")
    for sel in tree.find_all(exp.Select):
        names = {e.alias_or_name for e in sel.expressions}
        assert "b" not in names and "c" not in names, (
            f"pruning was disabled: {names} still projected"
        )


def test_unused_columns_pruned_when_outer_filters():
    tree = optimized("SELECT t.a FROM (SELECT a, b FROM x) t WHERE t.a > 1")
    for sel in tree.find_all(exp.Select):
        assert "b" not in {e.alias_or_name for e in sel.expressions}


def test_intersect_distinct_still_prunes_nothing_extra_is_kept():
    # Plain INTERSECT (distinct) semantics: operands must keep both projected
    # columns because row equality is over the projected tuple — behavior is
    # unchanged from before, pinned here both ways.
    sql = optimized_sql("SELECT q.a FROM (SELECT a, b FROM x INTERSECT SELECT a, b FROM y) q")
    assert '"b"' in sql


def test_plain_aggregate_roundtrip_unchanged():
    assert (
        optimized_sql("SELECT a, SUM(b) AS s FROM x GROUP BY a")
        == 'SELECT "x"."a" AS "a", SUM("x"."b") AS "s" FROM "x" AS "x" GROUP BY "x"."a"'
    )


def test_fully_used_aggregation_subquery_unchanged():
    tree = optimized("SELECT t.a, t.s FROM (SELECT a, SUM(b) AS s FROM x GROUP BY a) t")
    inner = [s for s in tree.find_all(exp.Select) if s.args.get("group")]
    assert inner
    assert {"a", "s"} <= {e.alias_or_name for e in inner[0].expressions}


def test_explicit_group_by_still_allows_pruning_unused_projections():
    # With explicit keys, an unused projection can be pruned without touching
    # the grouping — the optimizer must keep doing this.
    tree = optimized("SELECT t.s FROM (SELECT a, b, SUM(c) AS s FROM x GROUP BY a, b) t")
    inner = [s for s in tree.find_all(exp.Select) if s.args.get("group")]
    assert inner
    names = {e.alias_or_name for e in inner[0].expressions}
    assert "s" in names
    assert "b" not in names or "a" not in names or len(names) < 3

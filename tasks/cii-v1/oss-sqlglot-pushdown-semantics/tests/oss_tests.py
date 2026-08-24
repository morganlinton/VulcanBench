"""Hidden fail-to-pass tests: the optimizer must not change query semantics.

All assertions are structural properties of the optimized parse tree (which
projections survive inside a subquery/CTE or set-operation operand), never
exact SQL text, so any semantics-preserving optimization passes.
"""

from __future__ import annotations

from sqlglot import exp, parse_one
from sqlglot.optimizer import optimize

SCHEMA = {
    "x": {"a": "INT", "b": "INT", "c": "INT"},
    "y": {"a": "INT", "b": "INT", "c": "INT"},
    "events": {
        "user_id": "INT",
        "event": "TEXT",
        "ts": "TIMESTAMP",
        "amount": "INT",
        "region": "TEXT",
    },
}


def optimized(sql: str, dialect: str = "duckdb") -> exp.Expression:
    tree = optimize(parse_one(sql, read=dialect), schema=SCHEMA, dialect=dialect)
    # Round-trip through SQL so we grade what a user would actually receive.
    return parse_one(tree.sql(dialect=dialect), read=dialect)


def inner_selects(tree: exp.Expression) -> list[exp.Select]:
    """Every SELECT other than the outermost one (CTE bodies, derived tables,
    set-operation operands)."""
    outer = tree.find(exp.Select)
    return [s for s in tree.find_all(exp.Select) if s is not outer]


def projected_names(select: exp.Select) -> set[str]:
    return {e.alias_or_name for e in select.expressions}


def test_group_by_all_keeps_its_implicit_grouping_keys():
    # `GROUP BY ALL` derives its keys from the non-aggregate projections, so
    # removing an "unused" projection changes which columns are grouped by
    # and silently changes the aggregation's row set.
    tree = optimized("SELECT t.a FROM (SELECT a, b, SUM(b) AS s FROM x GROUP BY ALL) t")
    inner = [s for s in inner_selects(tree) if s.args.get("group")]
    assert inner, "the aggregation subquery must survive optimization"
    names = projected_names(inner[0])
    assert {"a", "b"} <= names, f"grouping keys were pruned: inner projects only {names}"


def test_group_by_all_rollup_totals_unchanged():
    tree = optimized(
        """
        SELECT r.region FROM (
          SELECT region, event, SUM(amount) AS total
          FROM events GROUP BY ALL
        ) r
        """
    )
    inner = [s for s in inner_selects(tree) if s.args.get("group")]
    assert inner
    names = projected_names(inner[0])
    assert {"region", "event"} <= names, (
        f"the rollup now groups by {names}: totals are computed over the wrong keys"
    )


def test_intersect_all_operands_keep_their_columns():
    # INTERSECT ALL matches whole rows with multiset semantics: dropping a
    # column from the operands changes which rows count as equal.
    tree = optimized("SELECT q.a FROM (SELECT a, b FROM x INTERSECT ALL SELECT a, b FROM y) q")
    operands = [s for s in tree.find_all(exp.Select) if isinstance(s.parent, (exp.Intersect, exp.SetOperation))]
    if not operands:  # operand selects may sit inside the set op's this/expression args
        setop = tree.find(exp.Intersect)
        assert setop is not None, "the INTERSECT ALL must survive optimization"
        operands = [setop.this, setop.expression]
    for op in operands:
        names = projected_names(op)
        assert {"a", "b"} <= names, (
            f"an INTERSECT ALL operand was narrowed to {names}: row equality changes"
        )


def test_except_all_operands_keep_their_columns():
    tree = optimized("SELECT q.a FROM (SELECT a, b FROM x EXCEPT ALL SELECT a, b FROM y) q")
    setop = tree.find(exp.Except)
    assert setop is not None, "the EXCEPT ALL must survive optimization"
    for op in (setop.this, setop.expression):
        sel = op if isinstance(op, exp.Select) else op.find(exp.Select)
        names = projected_names(sel)
        assert {"a", "b"} <= names, (
            f"an EXCEPT ALL operand was narrowed to {names}: multiset subtraction changes"
        )

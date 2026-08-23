"""Hidden fail-to-pass tests: Postgres JSON operators (->, ->>, #>, #>>, ?)
must parse at Postgres's binary-operator precedence tier — below + and -,
level with ||, left-associative — not as tight-binding column accessors.

All assertions compare parse-tree BINDING, never rendering style: the tree
for an expression must equal the tree for its explicitly-parenthesized
reading and differ from the wrong reading (compared through each tree's own
canonical SQL under the same dialect).
"""

from __future__ import annotations

from sqlglot import exp, parse_one


def canon(sql: str, dialect: str = "postgres") -> str:
    """Structural repr of the parse tree with grouping parens stripped, so two
    inputs compare equal iff they BIND the same way."""
    tree = parse_one(sql, read=dialect).transform(
        lambda n: n.this if isinstance(n, exp.Paren) else n
    )
    return repr(tree)


def assert_binds(sql: str, right: str, wrong: str, dialect: str = "postgres") -> None:
    got, want, bad = canon(sql, dialect), canon(right, dialect), canon(wrong, dialect)
    assert got == want, f"{sql!r} parsed as {got!r}, expected the {right!r} reading"
    assert got != bad, f"{sql!r} must not parse as the {wrong!r} reading"


def test_cast_binds_to_the_operand_not_the_extraction():
    assert_binds(
        "SELECT a #>> b::TEXT[]",
        "SELECT a #>> (b::TEXT[])",
        "SELECT (a #>> b)::TEXT[]",
    )
    assert_binds(
        "SELECT j -> k::TEXT",
        "SELECT j -> (k::TEXT)",
        "SELECT (j -> k)::TEXT",
    )
    assert_binds(
        "SELECT j ->> k::TEXT",
        "SELECT j ->> (k::TEXT)",
        "SELECT (j ->> k)::TEXT",
    )


def test_arithmetic_binds_tighter_than_json_operators():
    assert_binds(
        "SELECT a -> b + c",
        "SELECT a -> (b + c)",
        "SELECT (a -> b) + c",
    )


def test_subscript_binds_to_the_operand():
    assert_binds(
        "SELECT j -> k[1]",
        "SELECT j -> (k[1])",
        "SELECT (j -> k)[1]",
    )


def test_duckdb_shares_the_precedence_tier():
    assert_binds(
        "SELECT j -> k::TEXT",
        "SELECT j -> (k::TEXT)",
        "SELECT (j -> k)::TEXT",
        dialect="duckdb",
    )

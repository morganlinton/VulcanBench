"""Hidden pass-to-pass guards: everything around the precedence change that
must not move — lambda syntax in other dialects, ordinary precedence,
concatenation, casts, and cross-dialect transpilation."""

from __future__ import annotations

from sqlglot import exp, parse_one


def canon(sql: str, dialect: str) -> str:
    return parse_one(sql, read=dialect).sql(dialect=dialect)


def struct(sql: str, dialect: str = "postgres") -> str:
    tree = parse_one(sql, read=dialect).transform(
        lambda n: n.this if isinstance(n, exp.Paren) else n
    )
    return repr(tree)


# Held before the change too: chained extractions group left-to-right.
def test_json_operators_stay_left_associative():
    assert struct("SELECT a -> b -> c") == struct("SELECT (a -> b) -> c")
    assert struct("SELECT a -> b -> c") != struct("SELECT a -> (b -> c)")


def test_lambda_dialects_keep_arrow_as_lambda():
    assert canon("SELECT TRANSFORM(xs, x -> x + 1)", "databricks") == "SELECT TRANSFORM(xs, x -> x + 1)"
    assert canon("SELECT FILTER(xs, x -> x > 2)", "spark") == "SELECT FILTER(xs, x -> x > 2)"


def test_ordinary_precedence_unchanged():
    assert canon("SELECT a + b * c", "postgres") == "SELECT a + b * c"
    assert canon("SELECT a || b || c", "postgres") == "SELECT a || b || c"
    assert canon("SELECT CAST(a AS TEXT)", "postgres") == "SELECT CAST(a AS TEXT)"


def test_parenthesized_reading_survives_roundtrip():
    # An explicitly grouped extraction-then-add must stay grouped that way.
    e = parse_one("SELECT (a -> b) + c", read="postgres")
    again = parse_one(e.sql(dialect="postgres"), read="postgres")
    assert again.sql(dialect="postgres") == e.sql(dialect="postgres")


def test_question_operator_roundtrips():
    e = parse_one("SELECT j ? k", read="postgres")
    assert parse_one(e.sql(dialect="postgres"), read="postgres").sql(dialect="postgres") == e.sql(
        dialect="postgres"
    )


def test_postgres_to_duckdb_transpile_unchanged():
    assert (
        parse_one("SELECT j -> 'k'", read="postgres").sql(dialect="duckdb")
        == "SELECT j -> '$.k'"
    )

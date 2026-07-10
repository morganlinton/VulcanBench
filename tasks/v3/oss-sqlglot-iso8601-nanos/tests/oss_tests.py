"""Hidden tests for oss-sqlglot-iso8601-nanos.

Presto's FROM_ISO8601_TIMESTAMP_NANOS(<iso string>) has no equivalent function in
several target dialects, so transpiling it out of Presto must convert it to an
equivalent timestamp cast rather than emitting the (unrunnable) function call.
At the unfixed base the function is unknown to sqlglot and parses to a generic
Anonymous node, so it passes through verbatim in every dialect.
Expected outputs captured from the real upstream fix (sqlglot PR #7808).
"""
import sqlglot

INP = "SELECT FROM_ISO8601_TIMESTAMP_NANOS('2020-05-11T11:15:05.000000000')"
LIT = "'2020-05-11T11:15:05.000000000'"


# --- fail_to_pass ---

def test_nanos_transpiles_to_timestamptz_cast():
    for dialect in ("snowflake", "duckdb"):
        got = sqlglot.transpile(INP, read="presto", write=dialect)[0]
        assert got == f"SELECT CAST({LIT} AS TIMESTAMPTZ)", f"{dialect}: {got!r}"


def test_nanos_transpiles_to_timestamp_cast():
    for dialect in ("spark", "databricks", "bigquery"):
        got = sqlglot.transpile(INP, read="presto", write=dialect)[0]
        assert got == f"SELECT CAST({LIT} AS TIMESTAMP)", f"{dialect}: {got!r}"


def test_nanos_parses_to_dedicated_expression():
    # At base the unknown function parses to exp.Anonymous; the fix adds a
    # dedicated FromISO8601TimestampNanos expression class.
    tree = sqlglot.parse_one(INP, read="presto")
    funcs = list(tree.find_all(sqlglot.exp.Func))
    names = {type(f).__name__ for f in funcs}
    assert "FromISO8601TimestampNanos" in names, f"parsed as {names}"


# --- pass_to_pass (unchanged by the fix) ---

def test_unrelated_transpile_is_stable():
    assert sqlglot.transpile("SELECT 1", read="presto", write="spark")[0] == "SELECT 1"


def test_non_nanos_iso8601_timestamp_unaffected():
    # The sibling FROM_ISO8601_TIMESTAMP already casts correctly at base.
    inp = "SELECT FROM_ISO8601_TIMESTAMP('2020-05-11T11:15:05')"
    assert (
        sqlglot.transpile(inp, read="presto", write="duckdb")[0]
        == "SELECT CAST('2020-05-11T11:15:05' AS TIMESTAMPTZ)"
    )
    assert (
        sqlglot.transpile(inp, read="presto", write="spark")[0]
        == "SELECT CAST('2020-05-11T11:15:05' AS TIMESTAMP)"
    )

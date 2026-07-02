"""Hidden test for oss-sqlglot-iso8601-nanos.

Presto's FROM_ISO8601_TIMESTAMP_NANOS(<iso string>) has no equivalent function in
several target dialects, so transpiling it out of Presto must convert it to an
equivalent timestamp cast rather than emitting the (unrunnable) function call.
Expected outputs captured from the real upstream fix (sqlglot PR #7808).
"""
import sqlglot

INP = "SELECT FROM_ISO8601_TIMESTAMP_NANOS('2020-05-11T11:15:05.000000000')"
EXPECTED = {
    "snowflake": "SELECT CAST('2020-05-11T11:15:05.000000000' AS TIMESTAMPTZ)",
    "spark": "SELECT CAST('2020-05-11T11:15:05.000000000' AS TIMESTAMP)",
    "databricks": "SELECT CAST('2020-05-11T11:15:05.000000000' AS TIMESTAMP)",
    "bigquery": "SELECT CAST('2020-05-11T11:15:05.000000000' AS TIMESTAMP)",
    "duckdb": "SELECT CAST('2020-05-11T11:15:05.000000000' AS TIMESTAMPTZ)",
}


def test_from_iso8601_timestamp_nanos_transpiles_to_cast():
    for dialect, expected in EXPECTED.items():
        got = sqlglot.transpile(INP, read="presto", write=dialect)[0]
        assert got == expected, f"{dialect}: {got!r} != {expected!r}"


def test_unrelated_transpile_is_stable():
    # pass_to_pass no-regression anchor: unrelated SQL must be untouched.
    assert sqlglot.transpile("SELECT 1", read="presto", write="spark")[0] == "SELECT 1"

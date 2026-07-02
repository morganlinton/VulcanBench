"""Hidden test for oss-sqlglot-hive-json-roundtrip.

Hive PARSE_JSON and TO_JSON must round-trip (read hive, write hive) unchanged, rather than
being dropped or rewritten into a broken expression. Behavior captured from the upstream
fix (sqlglot PR #7818).
"""
import sqlglot


def _rt(q: str) -> str:
    return sqlglot.transpile(q, read="hive", write="hive")[0]


def test_parse_json_roundtrip():
    assert _rt("SELECT PARSE_JSON('{}')") == "SELECT PARSE_JSON('{}')"


def test_to_json_parse_json_roundtrip():
    assert _rt("SELECT TO_JSON(PARSE_JSON('{}'))") == "SELECT TO_JSON(PARSE_JSON('{}'))"


def test_simple_select_roundtrip():
    # pass_to_pass no-regression anchor.
    assert _rt("SELECT 1") == "SELECT 1"

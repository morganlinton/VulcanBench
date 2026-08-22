"""Hidden fail-to-pass tests: ANALYZE and DROP with several tables must parse
and round-trip. Public API (transpile identity), no error-text assertions."""

import sqlglot


def roundtrip(sql, dialect=None, expected=None):
    out = sqlglot.transpile(sql, read=dialect, write=dialect)[0]
    assert out == (expected or sql), out


def test_mysql_analyze_multiple_tables():
    roundtrip("ANALYZE TABLE t1, t2", "mysql")


def test_mysql_analyze_local_qualified_tables():
    roundtrip("ANALYZE LOCAL TABLE db.t1, db.t2, t3", "mysql")


def test_postgres_analyze_multiple_tables():
    roundtrip("ANALYZE t1, t2", "postgres")
    roundtrip("ANALYZE VERBOSE t1, t2", "postgres")


def test_drop_view_multiple():
    roundtrip("DROP VIEW a, b")


def test_tsql_drop_view_strips_catalog():
    roundtrip("DROP VIEW a.b.c, a.b.d", "tsql", expected="DROP VIEW b.c, b.d")


def test_drop_multiple_tables_ast_lists_all():
    expr = sqlglot.parse_one("DROP TABLE s.a, s.b, c")
    tables = expr.args.get("tables") or []
    names = sorted(t.sql() for t in tables)
    assert names == ["c", "s.a", "s.b"], names

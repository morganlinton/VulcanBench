"""Hidden pass-to-pass guards: single-table and already-working multi-table
DDL round trips must not regress."""

import sqlglot


def roundtrip(sql, dialect=None):
    out = sqlglot.transpile(sql, read=dialect, write=dialect)[0]
    assert out == sql, out


def test_drop_table_multiple_still_works():
    roundtrip("DROP TABLE a, b")
    roundtrip("DROP TABLE IF EXISTS a, b, c")


def test_single_table_drop_forms():
    roundtrip("DROP TABLE a")
    roundtrip("DROP VIEW v")
    roundtrip("DROP TABLE IF EXISTS s.t")


def test_single_table_analyze_forms():
    roundtrip("ANALYZE t1", "postgres")
    roundtrip("ANALYZE TABLE t1", "mysql")


def test_plain_select_untouched():
    roundtrip("SELECT a, b FROM t WHERE c = 1")

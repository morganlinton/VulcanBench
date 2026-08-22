# ANALYZE and DROP with multiple tables fail to parse (or mis-model the list)

Several databases accept a list of tables in `ANALYZE` and `DROP`, but
sqlglot either rejects the list or squeezes it into a lopsided AST:

```python
>>> import sqlglot
>>> sqlglot.transpile("ANALYZE TABLE t1, t2", read="mysql", write="mysql")
ParseError: Invalid expression / Unexpected token. Line 1, Col: 18.

>>> sqlglot.transpile("ANALYZE t1, t2", read="postgres", write="postgres")
ParseError: ...

>>> sqlglot.transpile("DROP VIEW a, b")
ParseError: Invalid expression / Unexpected token. Line 1, Col: 12.
```

`DROP TABLE a, b` happens to round-trip, but the expression tree puts the
first table in one slot and the rest in another, so consumers walking the
AST see an inconsistent shape.

Wanted:

- Multi-table `ANALYZE` parses and round-trips for the dialects that accept
  it (e.g. MySQL `ANALYZE TABLE t1, t2`, `ANALYZE LOCAL TABLE db.t1, db.t2,
  t3`; Postgres `ANALYZE t1, t2`, `ANALYZE VERBOSE t1, t2`).
- Multi-table `DROP` parses and round-trips for all object kinds (`DROP
  VIEW a, b`), and the `Drop`/`Analyze` expressions model the targets as one
  uniform table list.
- T-SQL's existing name normalization keeps applying to every listed table
  (`DROP VIEW a.b.c, a.b.d` → `DROP VIEW b.c, b.d`).
- Single-table forms, `IF EXISTS`, and everything else round-trip exactly
  as today.

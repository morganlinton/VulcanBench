# Optimized queries return different results than the originals

We run every analytics query through `sqlglot.optimizer.optimize` (with a
schema) before execution. After a recent audit we found the optimizer is
changing the *results* of some queries, not just their shape. Two incident
reports, both verified by running the original and optimized SQL side by
side on the same warehouse:

**Report 1 — revenue rollup returns inflated totals.** DuckDB.

```sql
SELECT r.region FROM (
  SELECT region, event, SUM(amount) AS total
  FROM events GROUP BY ALL
) r
```

The original returns one row per (region, event) pair. The optimized query
returns one row per region — the aggregation is being computed over
different grouping keys than the query specifies, so downstream joins on
the row count silently break.

**Report 2 — a reconciliation query loses rows.** DuckDB.

```sql
SELECT q.a FROM (SELECT a, b FROM x INTERSECT ALL SELECT a, b FROM y) q
```

The original computes the multiset intersection over (a, b) pairs; the
optimized query returns a different number of rows. The same happens with
`EXCEPT ALL`.

The optimizer must never change what a query computes. Whatever the root
cause, the fix must not simply turn optimizations off: unused-column
pruning demonstrably improves our warehouse bills and must keep working
wherever it is actually safe (plain subqueries, explicit `GROUP BY` with
genuinely unused projections, and so on).

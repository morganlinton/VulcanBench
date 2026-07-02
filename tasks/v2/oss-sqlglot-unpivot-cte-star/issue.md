sqlglot's query optimizer qualifies and expands `SELECT *`. When a query selects `*` from an `UNPIVOT` whose source is a CTE, the optimizer does not qualify it correctly.

Given, in BigQuery:

```sql
WITH produce AS (SELECT 'Kale' AS product, 51 AS q1, 23 AS q2)
SELECT * FROM produce UNPIVOT(sales FOR quarter IN (q1, q2))
```

qualification should:

- expand `SELECT *` to the columns visible after the UNPIVOT: the non-unpivoted column (`product`) plus the UNPIVOT-produced name and value columns (`quarter`, `sales`); and
- leave the UNPIVOT-generated column names (`sales`, `quarter`) unqualified, while the columns listed in the `IN (...)` clause are qualified against the source (`produce.q1`, `produce.q2`).

The expected result is:

```sql
WITH produce AS (SELECT 'Kale' AS product, 51 AS q1, 23 AS q2)
SELECT produce.product AS product, produce.quarter AS quarter, produce.sales AS sales
FROM produce AS produce UNPIVOT(sales FOR quarter IN (produce.q1, produce.q2)) AS produce
```

Instead, the buggy optimizer leaves `SELECT *` unexpanded and wrongly qualifies the UNPIVOT-generated `sales`/`quarter` columns against the source table.

Fix the column resolution so that `SELECT *` over an UNPIVOT of a CTE expands to include the UNPIVOT's output columns and does not source-qualify the generated column names. Plain star expansion over a CTE (without UNPIVOT) must remain unchanged.

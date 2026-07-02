sqlglot's query optimizer has a `qualify` step that resolves column references against a schema. When qualifying a Snowflake query that uses a `LATERAL FLATTEN(...)` table function whose source table is not present in the schema passed to `qualify`, the column resolver enters infinite recursion and never terminates (it surfaces as a `RecursionError`) instead of reporting that the column could not be resolved.

For example, qualifying

```sql
SELECT f.value AS v FROM my_db.raw.events, LATERAL FLATTEN(items) AS f
```

with a schema that does not contain `my_db.raw.events` recurses forever while trying to resolve the unqualified `items` column that feeds the lateral.

Fix the optimizer's column resolution so that an unresolvable lateral column raises a clean `OptimizeError` (the same way other unresolved columns already do) instead of recursing infinitely. Lateral queries whose columns can be resolved must continue to qualify correctly.

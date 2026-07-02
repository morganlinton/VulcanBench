Snowflake's `SELECT * ILIKE '<pattern>'` star filter is broken, and combining it with `RENAME` does not parse at all.

Snowflake lets a query filter the columns produced by a star projection by name pattern, optionally renaming some of the survivors:

```sql
SELECT * ILIKE 'a%' FROM x;
SELECT * ILIKE 'a%' RENAME (aa AS d) FROM x;
```

Two problems today:

1. Parsing the `ILIKE` and `RENAME` modifiers together fails with `ParseError: Invalid expression / Unexpected token` at the `RENAME` keyword.
2. A star with only the `ILIKE` filter parses, but the optimizer's `qualify` step treats it like an ordinary boolean `ILIKE` expression over values, producing something like `SELECT * ILIKE 'a%' AS "_col_0"` instead of expanding the star to the columns whose names match the pattern.

Expected behavior: the combined form parses and round-trips through the SQL generator, and qualification expands the star to exactly those schema columns whose names match the pattern case-insensitively (standard SQL LIKE wildcards), with `RENAME` aliases applied to the matching columns. Star modifiers that work today (`EXCEPT`, `REPLACE`, `RENAME` without `ILIKE`) must keep working unchanged.

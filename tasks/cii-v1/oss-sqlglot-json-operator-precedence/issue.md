# Postgres JSON operators parse at the wrong precedence

The Postgres operators `->`, `->>`, `#>`, `#>>` and `?` are being parsed
as tight-binding column accessors, like `.` and `::`. In Postgres they are
ordinary binary operators: they sit in the "any other operator" precedence
tier — below `+`/`-`, level with `||` — and are left-associative.

The mis-binding corrupts adjacent casts, subscripts and arithmetic:

```sql
SELECT a #>> b::TEXT[]
-- Postgres reads this as:  a #>> (b::TEXT[])
-- currently parsed as:     (a #>> b)::TEXT[]

SELECT a -> b + c
-- Postgres reads this as:  a -> (b + c)
-- currently parsed as:     (a -> b) + c

SELECT j -> k[1]
-- Postgres reads this as:  j -> (k[1])
-- currently parsed as:     (j -> k)[1]
```

DuckDB shares Postgres's precedence for these operators and shows the same
mis-binding. Dialects where `->` is lambda syntax (Spark, Databricks, etc.)
are unrelated and must keep their current behavior, as must ordinary
arithmetic precedence, `||` associativity, casts, and Postgres→DuckDB
transpilation of JSON extraction.

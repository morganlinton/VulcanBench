sqlglot's query optimizer has a `qualify` step whose resolver assigns each column reference to the source that provides it. When a query uses `LATERAL VIEW EXPLODE(...)` over an `ARRAY<STRUCT<...>>` column and then references the struct's fields through the lateral alias, the resolver fails to resolve those fields.

For example, in Spark, given a table `my_table` with column `items` of type `ARRAY<STRUCT<name STRING, age INT>>`:

```sql
SELECT ci.name, ci.age FROM my_table LATERAL VIEW EXPLODE(items) ci AS ci
```

should qualify to:

```sql
SELECT ci.name AS name, ci.age AS age FROM my_table AS my_table LATERAL VIEW EXPLODE(my_table.items) ci AS ci
```

Instead the buggy resolver raises `OptimizeError: Unknown column: name`, because the columns exposed by the exploded struct are not made available under the lateral alias.

Fix the resolver so that struct fields produced by a `LATERAL VIEW EXPLODE` of an array-of-struct column resolve correctly through the lateral alias. Plain column selects must be unaffected.

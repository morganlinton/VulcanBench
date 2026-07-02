sqlglot's query optimizer qualifies and expands `SELECT alias.*`. In BigQuery a table column can be a `STRUCT`, and `column.*` expands that struct's fields. There is a scope-resolution bug in how the optimizer chooses which source `alias.*` refers to.

When a query defines a CTE whose name matches a struct column, and that CTE is **not** referenced in the FROM clause, the optimizer incorrectly resolves `name.*` against the unreferenced CTE instead of the struct column, and fails.

For example, given a table `structs` that has a `STRUCT` column `one` with fields `a_1` and `b_1`:

```sql
WITH one AS (SELECT 1 AS x) SELECT one.* FROM structs
```

should expand to the struct's fields:

```sql
WITH one AS (SELECT 1 AS x) SELECT structs.one.a_1 AS a_1, structs.one.b_1 AS b_1 FROM structs AS structs
```

because the CTE `one` is not selected in the FROM clause. Instead, the buggy optimizer treats `one.*` as the CTE and raises a "column could not be resolved" error.

When the same-named CTE **is** in the FROM clause (`FROM structs, one`), it must take precedence and `one.*` expands to the CTE's columns.

Fix the column resolution so star expansion looks at the correct source set: a same-named CTE that is not selected in the query must not shadow a struct column.

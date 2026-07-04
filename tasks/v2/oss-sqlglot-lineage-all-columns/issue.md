The `lineage()` helper can only trace one output column at a time, and offers no way to observe or annotate the nodes it builds as it walks.

Extend `sqlglot.lineage.lineage()` with two capabilities:

1. All-columns mode. When the `column` argument is `None`, instead of tracing a single named column, trace every final output column of the query and return a mapping from each output column name to its lineage `Node`. Passing a specific column name keeps returning that column's single `Node` as before.

2. An `on_node` callback. Accept an optional `on_node` callable that is invoked once for every `Node` created during the lineage walk (the root output node and every upstream node it expands, across CTEs, subqueries, and set operations). This lets a caller collect or annotate nodes as the graph is built. It must fire in both single-column and all-columns modes.

Update the type overloads so that `lineage(column=None, ...)` is typed as returning the mapping and `lineage(column="name", ...)` as returning a single `Node`. Existing single-column lineage behavior, including walking upstream to base-table columns, must be unchanged.

Add an optimizer pass that rewrites a query into a canonical structural form, so that two queries which are semantically the same shape over the same data produce identical SQL, while queries that differ in their data contract stay distinct.

The idea: a query's *data contract* is the set of real, externally-meaningful names, the physical base tables it reads, the base-table columns it references, and the output aliases the caller sees. Everything else (table aliases, CTE and subquery names, the aliases of intermediate projections) is an *internal handle* whose specific spelling is irrelevant to what the query means.

Add `sqlglot.optimizer.canonicalize_internal_names.canonicalize_internal_names(expression)` that assumes the expression has already been through `qualify` (columns bound to sources, aliases present on every projection, subquery, and CTE) and rewrites it so that:

- Base-table names and base-table column names are preserved.
- Top-level output aliases (the query's result columns) are preserved. For a set operation the top-level output names come from the leftmost leaf SELECT (except `UNION BY NAME`, where both branches contribute).
- Every internal handle is renamed to a sequential canonical name: table aliases and CTE/subquery names to `_t0`, `_t1`, ..., and internal column aliases to `_c0`, `_c1`, ..., assigned in a deterministic traversal order.

The result is that two structurally-equivalent queries canonicalize to byte-identical SQL, but a query reading a different physical table, or a different base column, canonicalizes differently. Existing optimizer passes, including `qualify` on its own, must be unaffected.

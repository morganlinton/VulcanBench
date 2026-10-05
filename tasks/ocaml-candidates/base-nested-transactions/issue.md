# Nested logical transactions in Base.Hashtbl

Add the supplied Hashtbl.with_transaction API to this complete pinned Base repository.
The prototype simply calls its callback. Implement table-local savepoints across the
existing mutation paths, including the direct AVL bulk-update path. This is an
original feature request, not a claim of an upstream defect. Public client modules
exercise cache updates, bulk normalization, and nested imports; keep them working.

A successful callback returns its exact result and commits its changes. If it raises,
restore the table bindings and length as they were at this invocation's entry, then
re-raise the same exception object. Successful nested transactions remain undoable by
their enclosing transaction. A failing inner transaction restores only its own edits,
even when the outer callback catches that exception and continues. State is per table:
copies and other tables do not inherit the caller's transaction history.

Cover set, add/add_exn, remove/find_and_remove, clear, change, update/update_and_return,
find_or_add/findi_or_add, incr/decr, multi-value operations, map/mapi_inplace,
filter/filter_map variants, and merge_into destination edits. Ordinary behavior
outside a transaction stays unchanged. A mapping callback may fail after earlier
entries changed; rollback restores those entries. Existing iteration mutation guards
still apply: entering a transaction while mutation is forbidden must raise without
running f. Later mutations must remain enabled after an exception.

Restore the original stored key representatives under a custom comparator/hash and
the original data objects (physical identity). Values themselves and external side
effects are not deep-rolled-back. Key hash/compare functions are total, stable, and
non-mutating. Capacity and AVL shape may remain changed after resizing; observable
contents, key representatives, lengths, and invariants must be restored.

Use a sparse undo strategy, not eager copies or a full-table scan for every savepoint.
On a 50000-entry table with ample capacity and no resizing, a transaction replacing
one existing binding then raising must allocate less than 262144 bytes and make at
most 1024 total hash/comparison calls, measured only during that transaction.
A two-level nested savepoint touching two bindings must meet the same bounds.
Bulk operations and ordinary resizing may do their normal linear work.
Preserve every supplied .mli and src/hashtbl_intf.ml. No unsafe casts or hidden
global transaction registry. Build with the documented release profile; upstream
development warning settings are not the task's behavior contract.

# Requirement-to-check mapping

All requirements appear in the corresponding issue.md before measurement.
The grouped assertions below are functional checks, not quality ratings.
Every task also compiles public clients and preserves every supplied .mli.

| Task | Behavior group | Stated contract exercised |
|---|---|---|
| Durable delivery | keyed_dispatch | Global slots, per-key admission order, bypass of blocked keys, capacity through durability |
| Durable delivery | ordered_durability | Out-of-order worker completion, one journal writer, ordered acknowledgement, cancellation commit point |
| Durable delivery | retry_ownership | Backoff retains its key, other keys progress, correct attempt numbering |
| Durable delivery | timeout_fences | Equality deadline, stale callbacks, generation and slot ownership, alarm retirement |
| Durable delivery | cancel_commitpoint | Queued, running and backoff cancellation; no cancellation after journal submission |
| Durable delivery | close_abort | Graceful drain and abort during an outstanding journal append |
| Durable delivery | reentrant_callbacks | Synchronous worker and journal reentry through abort, submit, close and cancellation |
| Durable delivery | monitor_failures | Worker retries and terminal journal failure for synchronous/asynchronous exceptions |
| Durable delivery | seeded_delivery | Fifty deterministic admitted deliveries preserve result, order and shutdown invariants |
| Typed schemas | nested_migration | Legacy defaults, heterogeneous nested records, lists, pairs, options, archive client |
| Typed schemas | canonical_compatibility | Thirty independently packed legacy fixtures, signed extrema, entry permutation, round trips |
| Typed schemas | structured_errors | Exact absolute offsets and paths, trailing data, malformed scalars, framing truncation, duplicate unknown IDs, missing required fields |
| Typed schemas | transactional_cursor | Every truncation prefix, invalid bounds, nonzero start, empty Text, returned errors and raising constructor rollback |
| Typed schemas | unknown_field_budget | Skip a 2 MiB opaque field within the explicit 65536-byte allocation ceiling |
| Typed schemas | schema_validation | Duplicate/invalid typed schema IDs rejected before cursor mutation |
| Base transactions | rollback_mutators | Existing scalar and bulk mutation routes, including direct AVL updates and multi-value helpers |
| Base transactions | nested_savepoints | Inner commit remains undoable; inner failure can be caught while outer work commits |
| Base transactions | callback_errors | Partial bulk callback failure, existing iteration guard, mutation reenabled after exception |
| Base transactions | canonical_keys | Colliding custom keys and physical identity of stored keys and data |
| Base transactions | resize_clear_copy | Growth, clear and copies preserve contents, length and table-local state |
| Base transactions | sparse_journal | 50000 entries, two touched bindings, measured allocation and hash/comparison ceiling |

The baseline must pass ordinary_workflow and supplied_interfaces for every task.
The schema guard also rejects unsafe cast and Marshal shortcuts in original
library modules. Base's hashtbl_intf.ml is guarded because it defines the public
signature through a .ml file. Hidden tests cannot be satisfied by altering it.

## Compiling faulty controls

| Task | Control | Intended semantic rejection |
|---|---|---|
| Durable delivery | accepts-stale-worker | An old worker callback publishes after losing its generation |
| Durable delivery | publishes-before-durable-ack | A ticket settles before the journal acknowledges it |
| Durable delivery | close-stops-accepted-work | Graceful close prevents draining accepted jobs |
| Typed schemas | commits-before-validation | Cursor moves despite malformed input |
| Typed schemas | defaults-malformed-present-field | A malformed present field is incorrectly replaced by its default |
| Typed schemas | materializes-unknown-payload | Opaque skipped data is copied and exceeds the allocation contract |
| Base transactions | discards-inner-commit | A successful inner savepoint cannot be undone by its parent |
| Base transactions | eager-table-snapshot | Functional rollback succeeds but sparse allocation bounds fail |

Controls must compile and preserve regression guards. A compiler or setup
failure cannot establish rejection. Each reference is checked in three fresh
offline workspaces. Model failures are reported under every original group
name; partial scores do not become whole-task passes.

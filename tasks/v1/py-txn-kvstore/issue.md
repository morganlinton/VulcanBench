# Implement a transactional in-memory key/value store

The `txnkv` package should provide a `Store` with nested transactions, backed by
an `UndoJournal`. The work is split across two files and both must be implemented
and kept consistent:

- `txnkv/journal.py` — `UndoJournal` owns the stack of transaction frames and the
  commit/rollback nesting semantics.
- `txnkv/store.py` — `Store` owns the data and computes the inverse of each
  mutation, driving the journal.

## `Store` API

- `get(name)`: return the current value, or raise `KeyError` if the key is absent.
- `set(key, value)`: set `key` to `value`.
- `delete(key)`: remove `key`, or raise `KeyError` if it is absent.
- `keys()`: return the current set of keys.
- `begin()`: open a transaction. Transactions may nest.
- `commit()`: commit the innermost open transaction. Raise `RuntimeError` if no
  transaction is open.
- `rollback()`: roll back the innermost open transaction, restoring the exact
  state from just before its `begin()`. Raise `RuntimeError` if none is open.

## Required behavior

- **Autocommit.** Mutations made with no open transaction take effect immediately
  and are durable (they cannot be rolled back later).
- **Exact rollback.** Rolling back restores the prior state precisely: a key that
  was overwritten returns to its old value, a key that did not exist before the
  transaction is removed again, and a deleted key is restored.
- **Reverse order.** When a key is written several times in one transaction,
  rollback must undo the writes in the correct order so the original value
  returns.
- **Nested commit folds into the parent.** Committing a nested transaction keeps
  its changes, but they must remain undoable by an outer `rollback()` — an outer
  rollback reverts everything done since the outer `begin()`, including work that
  inner transactions committed.
- **Nested rollback is isolated.** Rolling back an inner transaction must not
  disturb the outer transaction's pending changes.

## `UndoJournal` contract

`Store` records an *inverse operation* (a zero-argument callable that reverses one
mutation) for every change. Implement the journal so the two cooperate:

- `in_transaction()`: whether any frame is currently open.
- `push()`: open a new (nested) frame.
- `record(undo)`: record an inverse into the current frame. If no frame is open,
  the mutation is durable, so nothing is recorded.
- `rollback()`: run the current frame's inverses in reverse order, then pop it.
- `commit()`: pop the current frame and fold its inverses into the parent
  (preserving order) so a later parent rollback still undoes them; at the top
  level there is no parent, so the changes become durable.

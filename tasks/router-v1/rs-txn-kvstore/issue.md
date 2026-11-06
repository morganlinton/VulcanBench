# Implement the transactional key/value store

The `txnkv` crate should provide a `Store` with nested transactions, backed
by an `UndoJournal`. The types, signatures and documentation are in place;
the bodies that matter are `todo!()` stubs. Both files must be implemented
and kept consistent:

- `src/journal.rs`: `UndoJournal` owns the stack of transaction frames and
  the commit/rollback nesting semantics.
- `src/store.rs`: `Store` owns the data and computes the inverse of each
  mutation, driving the journal.

Keep the public signatures as they are; the tests use them.

## `Store` API

- `get(&self, key) -> Result<&str, StoreError>`: the current value, or
  `Err(MissingKey(key))`.
- `set(&mut self, key, value)`: set `key` to `value`.
- `delete(&mut self, key) -> Result<(), StoreError>`: remove `key`, or
  `Err(MissingKey(key))` if it is absent.
- `keys(&self) -> BTreeSet<String>`: the current keys.
- `begin(&mut self)`: open a transaction. Transactions may nest.
- `commit(&mut self) -> Result<(), StoreError>`: commit the innermost open
  transaction; `Err(NoTransaction)` if none is open.
- `rollback(&mut self) -> Result<(), StoreError>`: roll back the innermost
  open transaction, restoring the exact state from just before its
  `begin()`; `Err(NoTransaction)` if none is open.
- `in_transaction(&self) -> bool`.

## Required behaviour

- **Autocommit.** Mutations made with no open transaction take effect
  immediately and are durable; a later `rollback()` is `Err(NoTransaction)`
  and changes nothing.
- **Exact rollback.** Rolling back restores the prior state precisely: a key
  that was overwritten returns to its old value, a key that did not exist
  before the transaction is removed again, and a deleted key is restored.
- **Reverse order.** When a key is written (or deleted) several times in one
  transaction, rollback undoes the mutations in reverse order so the original
  value returns.
- **Nested commit folds into the parent.** Committing a nested transaction
  keeps its changes, but they must remain undoable by an outer `rollback()`:
  an outer rollback reverts everything done since the outer `begin()`,
  including work that inner transactions committed, however deep.
- **Nested rollback is isolated.** Rolling back an inner transaction must not
  disturb the outer transaction's pending changes.

## `UndoJournal` contract

`Store` records an *inverse operation* for every change: an `Undo`, a boxed
`FnOnce(&mut Data)` that is handed the data map when it runs and reverses
one mutation exactly. The journal never borrows the store; it is given the
map at rollback time. Implement the journal so the two cooperate:

- `in_transaction()`: whether any frame is open.
- `push()`: open a new (nested) frame.
- `record(undo)`: record an inverse into the current frame. If no frame is
  open, the mutation is durable, so the inverse is dropped.
- `rollback(&mut self, data)`: run the current frame's inverses against
  `data` in reverse order, then drop the frame; `false` if no frame is open.
- `commit()`: drop the current frame and fold its inverses into the parent
  frame (preserving order) so a later parent rollback still undoes them; at
  the top level there is no parent, so the changes become durable; `false`
  if no frame is open.

Run `cargo test --offline` to check your work; the crate has no dependencies.

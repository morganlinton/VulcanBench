# rs-txn-kvstore: builder notes

Rust counterpart of `tasks/v1/py-txn-kvstore`, authored 2026-11-05 for the
Router v1 suite's Rust small-hard cell (no public task fills it).

- Starting repo: types, signatures and docs are complete; `UndoJournal`'s
  frame methods and `Store`'s mutation/transaction methods are `todo!()`
  stubs that compile. No dependencies.
- The Rust twist on the Python original: an inverse is a boxed
  `FnOnce(&mut Data)` that receives the data map when it runs, so the
  journal never holds a borrow of the store and `Store::rollback` can hand
  `&mut self.data` to `self.journal.rollback` (disjoint fields).
- Gold: 135-line diff. Hidden tests: `tests/tests/test_txnkv.rs` (10
  fail-to-pass, 1 pass-to-pass `construct`); the deep-nesting test checks
  that every committed level folds into its parent.
- Verified locally with cargo 1.87 (brew rustup): gold passes 11/11, the
  stub compiles and every fail-to-pass test panics on `todo!()`.

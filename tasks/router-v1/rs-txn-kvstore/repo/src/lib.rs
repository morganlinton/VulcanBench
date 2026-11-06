//! `txnkv`: an in-memory key/value store with nested transactions.
//!
//! [`store::Store`] owns the data and computes the inverse of every mutation;
//! [`journal::UndoJournal`] owns the stack of transaction frames and the
//! commit/rollback nesting semantics. The two cooperate through the
//! [`journal::Undo`] closure type: an inverse is a one-shot closure that is
//! handed the data map when it runs, so the journal never holds a borrow of
//! the store.

pub mod journal;
pub mod store;

pub use journal::{Data, Undo, UndoJournal};
pub use store::{Store, StoreError};

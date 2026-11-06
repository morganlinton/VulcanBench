//! The store: owns the data, computes inverses, drives the journal.

use std::collections::BTreeSet;

use crate::journal::{Data, UndoJournal};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// `get` or `delete` on a key that is not present.
    MissingKey(String),
    /// `commit` or `rollback` with no open transaction.
    NoTransaction,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::MissingKey(k) => write!(f, "no such key `{k}`"),
            StoreError::NoTransaction => write!(f, "no transaction is open"),
        }
    }
}

impl std::error::Error for StoreError {}

#[derive(Default)]
pub struct Store {
    data: Data,
    journal: UndoJournal,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current value for `key`, or `MissingKey`.
    pub fn get(&self, key: &str) -> Result<&str, StoreError> {
        self.data
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| StoreError::MissingKey(key.to_string()))
    }

    /// Set `key` to `value`, recording how to undo it.
    pub fn set(&mut self, key: &str, value: &str) {
        let _ = (key, value);
        todo!()
    }

    /// Delete `key` (`MissingKey` if absent), recording how to undo it.
    pub fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        let _ = key;
        todo!()
    }

    /// The current set of keys, sorted.
    pub fn keys(&self) -> BTreeSet<String> {
        self.data.keys().cloned().collect()
    }

    /// Open a new transaction (transactions may nest).
    pub fn begin(&mut self) {
        todo!()
    }

    /// Commit the innermost open transaction.
    pub fn commit(&mut self) -> Result<(), StoreError> {
        todo!()
    }

    /// Roll back the innermost open transaction, restoring the exact state
    /// from just before its `begin()`.
    pub fn rollback(&mut self) -> Result<(), StoreError> {
        todo!()
    }

    /// Whether a transaction is open.
    pub fn in_transaction(&self) -> bool {
        self.journal.depth() > 0
    }
}

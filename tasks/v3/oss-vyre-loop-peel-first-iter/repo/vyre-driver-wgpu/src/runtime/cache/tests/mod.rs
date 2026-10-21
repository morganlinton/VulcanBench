//! Runtime cache test suites.
//!
//! The `unit` child wires in per-subsystem unit tests (LRU, buffer pool
//! rotation, tiered policy transitions) via `explicit_mod_list!`.

pub mod unit;

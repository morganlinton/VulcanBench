//! Shared IR fragments used by megakernel builders and schedulers.

use vyre_foundation::ir::{Expr, Node};

/// Emits a relaxed atomic load.
///
/// target-text atomics are implicitly relaxed; explicit acquire/release ordering is
/// modeled by surrounding synchronization nodes, not by the atomic expression.
pub fn atomic_load_relaxed(buffer: &str, index: Expr) -> Expr {
    Expr::atomic_add(buffer, index, Expr::u32(0))
}

/// Emits a relaxed atomic store.
///
/// The returned node binds the previous value so callers can splice the store
/// into expression-only IR regions without losing the exchange result.
pub fn atomic_store_relaxed(name: &str, buffer: &str, index: Expr, value: Expr) -> Node {
    Node::let_bind(name, Expr::atomic_exchange(buffer, index, value))
}

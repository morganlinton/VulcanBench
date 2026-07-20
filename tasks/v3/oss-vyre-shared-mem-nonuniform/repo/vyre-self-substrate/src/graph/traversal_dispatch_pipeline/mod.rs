//! Self-substrate wrappers for graph traversal and fixpoint-step primitives.
//!
//! Graph traversal semantics stay in `vyre-primitives`, while self-substrate
//! owns the dispatch-facing names used by scheduler, resident fixed-point,
//! and parity code.

mod dispatch;
#[cfg(any(test, feature = "cpu-parity"))]
mod references;
#[cfg(test)]
mod tests;

pub use dispatch::*;
#[cfg(any(test, feature = "cpu-parity"))]
pub use references::*;

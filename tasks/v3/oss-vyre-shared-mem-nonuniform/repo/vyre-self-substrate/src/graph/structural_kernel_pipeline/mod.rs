//! Self-substrate wrappers for structural graph, causal, and logic kernels.
//!
//! The primitive crate owns the graph algorithms. This module owns the
//! self-hosting surface that wires those algorithms into scheduler,
//! optimizer, causal-analysis, knowledge-compilation, and resident traversal
//! contexts without forking their semantics.

mod dispatch;
#[cfg(any(test, feature = "cpu-parity"))]
mod references;
#[cfg(test)]
mod tests;

pub use dispatch::*;
#[cfg(any(test, feature = "cpu-parity"))]
pub use references::*;

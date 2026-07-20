//! Resident adaptive sparse/dense graph traversal.
//!
//! This module wires `reduce_count` and
//! `graph::adaptive_traverse::adaptive_sparse_dense_step` into resident
//! CUDA-ready sequences. Traversal semantics stay in `vyre-primitives`; this
//! facade owns resident scratch, layout identity, and stable public re-exports.

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
mod resident;
mod resident_steps;
mod state;
#[cfg(test)]
mod tests;
mod upload;

#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::*;
pub use resident::{
    ResidentAdaptiveFourRussiansDenseGraph, ResidentAdaptiveSparseQueueGraph,
    ResidentAdaptiveTraversalGraph,
};
pub use resident_steps::*;
pub use state::{AdaptiveTraversalPlanCacheSnapshot, AdaptiveTraversalResidentScratch};
pub use upload::*;
pub use vyre_primitives::graph::adaptive_traverse::{
    select_adaptive_traversal_mode, select_dense_traversal_kernel, AdaptiveTraversalMode,
    DenseTraversalKernel,
};

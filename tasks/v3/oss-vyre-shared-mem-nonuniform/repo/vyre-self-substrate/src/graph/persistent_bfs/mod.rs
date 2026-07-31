//! Multi-step BFS frontier expansion substrate consumer.
//!
//! Wires `vyre_primitives::graph::persistent_bfs` so the optimizer can
//! compute multi-step reachability in a single primitive call instead
//! of looping `csr_forward_traverse` by hand. The primitive accumulates
//! into `frontier_out` via OR and reports a sticky changed-flag, so the
//! caller knows whether any new nodes were added across all steps.

mod dispatch;
#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
mod resident;
mod state;
#[cfg(test)]
mod tests;

pub use dispatch::*;
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::*;
pub use resident::*;
pub use state::{
    PersistentBfsGpuScratch, PersistentBfsPlanCacheSnapshot, PersistentBfsResidentScratch,
    ResidentBfsGraph,
};

#[cfg(test)]
use resident::ensure_resident_query_handles;

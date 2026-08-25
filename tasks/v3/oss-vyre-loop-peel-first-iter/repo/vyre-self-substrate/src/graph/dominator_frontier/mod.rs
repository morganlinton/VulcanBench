//! Region-graph dominance-frontier substrate consumer.
//!
//! Wires `vyre_primitives::graph::dominator_frontier` into the dispatch
//! path. The dominator tree of a Region graph identifies which Region's
//! writes a Region depends on; the dominance frontier of a Region set
//! tells the optimizer where phi-style merges (or vyre's analogue:
//! per-Region buffer reconcile) must run.
//!
//! # The self-use
//!
//! Vyre's optimizer needs to know, for any seed set of Regions, the
//! Regions where their effects MUST be reconciled. The classic SSA
//! answer is the dominance frontier: where two paths from the seed
//! merge into a node not strictly dominated by any seed. Same query,
//! same primitive, different IR.
//!
//! # Composition
//!
//! [`compute_dominance_frontier`] takes CSR-encoded dominance closure,
//! predecessor lists, and a seed bitset, and returns the frontier
//! bitset. The CSR encoding matches the primitive's contract exactly,
//! so the substrate call is a one-liner that bumps the observability
//! counter and forwards.

mod dispatch;
#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
#[cfg(test)]
mod tests;

use vyre_primitives::graph::dominator_frontier::{
    frontier_size as primitive_frontier_size, DominatorFrontierProgramShape,
    DominatorFrontierStaticInputKey,
};

use crate::graph::dispatch_bridge::{CachedProgram, ProgramCache};

pub use dispatch::{
    compute_dominance_frontier_via, compute_dominance_frontier_via_into,
    compute_dominance_frontier_via_with_scratch_into,
};
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{compute_dominance_frontier, try_compute_dominance_frontier};

/// Caller-owned GPU dispatch scratch for dominance-frontier queries.
#[derive(Debug, Default)]
pub struct DominanceFrontierGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<DominatorFrontierProgramShape, CachedDominanceFrontierProgram>,
    static_input_key: Option<DominatorFrontierStaticInputKey>,
}

type CachedDominanceFrontierProgram = CachedProgram;

impl DominanceFrontierGpuScratch {
    #[cfg(test)]
    fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }
}

/// Number of Regions flagged in the frontier bitset. Useful as a
/// dispatch-time telemetry value: a high frontier count on a small
/// seed indicates a wide-merge program shape that fusion passes
/// should leave alone.
#[must_use]
pub fn frontier_size(frontier: &[u32]) -> u32 {
    primitive_frontier_size(frontier)
}

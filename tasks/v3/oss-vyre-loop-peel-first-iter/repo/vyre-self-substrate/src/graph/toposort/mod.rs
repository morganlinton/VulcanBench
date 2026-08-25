//! DAG topological-sort dispatch wrapper.
//!
//! Wires `vyre_primitives::graph::toposort::toposort` (zero prior
//! consumers) and `reachable::reachable` so the optimizer's pass
//! scheduler / megakernel scheduler / dispatch ordering can rely on
//! the same primitive shipped to user dialects. Replaces ad-hoc
//! Kahn's-algorithm reimplementations the optimizer carried inline.

mod dispatch;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests;

use crate::graph::dispatch_bridge::{CachedProgram, ProgramCache};
use vyre_primitives::graph::toposort::ToposortCsrStaticInputKey;

pub use dispatch::{
    topo_order_csr_via, topo_order_csr_via_with_scratch, topo_order_csr_via_with_scratch_into,
};
#[cfg(test)]
pub use reference::{reference_all_reachable, reference_reachable_set, reference_topo_order};

/// Caller-owned GPU dispatch scratch for topological-sort CSR queries.
#[derive(Debug, Default)]
pub struct ToposortGpuScratch {
    inputs: Vec<Vec<u8>>,
    program_cache: ProgramCache<u32, CachedToposortProgram>,
    static_input_key: Option<ToposortCsrStaticInputKey>,
}

type CachedToposortProgram = CachedProgram;

impl ToposortGpuScratch {
    #[cfg(test)]
    fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }
}

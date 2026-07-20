//! Region-graph motif-matching substrate consumer.
//!
//! Wires `vyre_primitives::graph::motif` so the optimizer can
//! pattern-match small Region shapes (e.g. "load-store-store" or
//! "atomic-then-barrier") for lint/audit/rewrite passes. Same
//! primitive graph-analysis surface ships to user dialects, now consumed by
//! vyre's own IR walker.

mod dispatch;

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;

#[cfg(test)]
mod tests;

pub use dispatch::{
    match_motif_via, match_motif_via_into, match_motif_via_with_scratch_into, motif_matches_via,
    motif_participation_count_via,
};

#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{
    match_motif, motif_matches, motif_participation_count, try_match_motif, try_motif_matches,
    try_motif_participation_count,
};

use vyre_foundation::ir::Program;
use vyre_primitives::graph::motif::{MotifLayout, MotifProgramCacheKey, MotifStaticInputKey};

use crate::graph::dispatch_bridge::ProgramCache;

/// Caller-owned GPU dispatch scratch for motif matching.
#[derive(Debug, Default)]
pub struct MotifGpuScratch {
    inputs: Vec<Vec<u8>>,
    motif_hits: Vec<u32>,
    static_input_key: Option<MotifStaticInputKey>,
    program_cache: ProgramCache<MotifProgramCacheKey, CachedMotifProgram>,
}

#[derive(Debug)]
struct CachedMotifProgram {
    layout: MotifLayout,
    program: Program,
}

impl MotifGpuScratch {
    #[cfg(test)]
    fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }
}

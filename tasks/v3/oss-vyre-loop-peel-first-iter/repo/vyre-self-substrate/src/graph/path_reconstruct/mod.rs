//! Path-reconstruction substrate consumer.
//!
//! Wires `vyre_primitives::graph::path_reconstruct` so the optimizer can
//! recover an explicit walk from a parent vector. Used by call-graph
//! diagnostics (which path led from entry to a region flagged by an analysis
//! pass), megakernel chain reconstruction, and schedule-explanation telemetry.
//!
//! Per the primitive's spec: walks parent links from `target` back to
//! the root (a node whose parent points at itself), writing the
//! materialized path into a caller-provided scratch buffer and
//! returning its length.

mod dispatch;

pub use dispatch::{
    path_to_root_via, reconstruct_path_via, reconstruct_path_via_with_scratch,
    reconstruct_paths_via, reconstruct_paths_via_with_scratch_into,
};

use crate::graph::dispatch_bridge::{CachedProgram, ProgramCache};
#[cfg(test)]
use vyre_foundation::ir::Program;
use vyre_primitives::graph::path_reconstruct::PathReconstructStaticInputKey;

#[cfg(test)]
mod reference;
#[cfg(test)]
use reference::path_reconstruct_cpu;
#[cfg(test)]
pub use reference::{reference_path_to_root, reference_reconstruct_path};

/// Caller-owned GPU dispatch scratch for path reconstruction.
#[derive(Debug, Default)]
pub struct PathReconstructGpuScratch {
    inputs: Vec<Vec<u8>>,
    len_out: Vec<u32>,
    static_input_key: Option<PathReconstructStaticInputKey>,
    single_program_cache: ProgramCache<u32, CachedSinglePathProgram>,
    batched_program_cache: ProgramCache<(u32, u32), CachedBatchedPathProgram>,
}

type CachedSinglePathProgram = CachedProgram;
type CachedBatchedPathProgram = CachedProgram;

impl PathReconstructGpuScratch {
    #[cfg(test)]
    fn single_program_builds(&self) -> usize {
        self.single_program_cache.builds()
    }

    #[cfg(test)]
    fn batched_program_builds(&self) -> usize {
        self.batched_program_cache.builds()
    }
}

#[cfg(test)]
mod tests;

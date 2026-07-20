//! Region-graph bidirectional one-step reach substrate consumer.
//!
//! Wires `vyre_primitives::graph::csr_bidirectional` into the dispatch
//! path. One bidirectional BFS step is the right primitive when the
//! optimizer wants the "neighborhood" of a Region  -  both writers
//! (predecessors) and readers (successors) at once. Used by
//! alias-class merging and the buffer-residency planner.

mod closure;
mod dispatch;
pub use closure::{
    bidirectional_closure_via, bidirectional_closure_via_into,
    bidirectional_closure_via_with_scratch_into,
};
pub use dispatch::{
    bidirectional_step_via, bidirectional_step_via_into, bidirectional_step_via_with_scratch_into,
};

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{
    reference_bidirectional_closure, reference_bidirectional_closure_into,
    reference_bidirectional_step,
};
#[cfg(test)]
use reference::{reference_csr_bidir, reference_csr_bidir_closure};
#[cfg(test)]
use vyre_primitives::graph::csr_bidirectional::can_dispatch_edge_buffers_without_padding;

use crate::graph::dispatch_bridge::{CachedProgram, ProgramCache};
use vyre_primitives::graph::csr_bidirectional::{
    CsrBidirectionalProgramKey, CsrBidirectionalStaticInputKey,
};

/// Caller-owned GPU dispatch scratch for bidirectional CSR traversal.
#[derive(Debug, Default)]
pub struct BidirectionalGpuScratch {
    inputs: Vec<Vec<u8>>,
    static_input_key: Option<CsrBidirectionalStaticInputKey>,
    program_cache: ProgramCache<CsrBidirectionalProgramKey, CachedBidirectionalProgram>,
}

type CachedBidirectionalProgram = CachedProgram;

impl BidirectionalGpuScratch {
    #[cfg(test)]
    fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }
}

#[cfg(test)]
mod edge_buffer_copy_tests {
    use super::*;

    #[test]
    fn non_empty_canonical_edges_do_not_need_padding_copy() {
        assert!(can_dispatch_edge_buffers_without_padding(3, 3, 3));
    }

    #[test]
    fn empty_edges_keep_one_word_padding_contract() {
        assert!(!can_dispatch_edge_buffers_without_padding(0, 0, 1));
    }

    #[test]
    fn mismatched_edge_arrays_never_take_zero_copy_path() {
        assert!(!can_dispatch_edge_buffers_without_padding(3, 2, 3));
        assert!(!can_dispatch_edge_buffers_without_padding(2, 3, 3));
    }
}

#[cfg(test)]
mod tests;

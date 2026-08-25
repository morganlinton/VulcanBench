//! Exploded-supergraph (IFDS encoding) substrate consumer.
//!
//! Wires the primitive-owned exploded-supergraph reference builder (zero
//! prior consumers) into the substrate so the optimizer can build
//! interprocedural-dataflow graphs directly. The IFDS encoding packs
//! `(proc_id, block_id, fact_id)` into a u32 node id, then composes
//! intra-/inter-procedural edges + GEN/KILL flow into a CSR ready for
//! reachability/closure analysis.

mod dispatch;

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;

#[cfg(test)]
mod tests;

pub use dispatch::{
    build_ifds_csr_via, build_ifds_csr_via_into, build_ifds_csr_via_with_scratch_into,
};

#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{
    reference_build_ifds_csr, reference_canonicalize_csr_within_rows, try_reference_build_ifds_csr,
};

use vyre_primitives::graph::exploded::{
    dense_to_encoded, encoded_to_dense, ifds_node_count_saturating, IfdsCsrProgramCacheKey,
    IfdsCsrRuleColumns, IfdsCsrRuleInputFingerprint, IfdsCsrStaticInputKey,
};

use crate::graph::dispatch_bridge::{CachedProgram, ProgramCache};

/// Caller-owned GPU dispatch scratch for exploded IFDS CSR construction.
#[derive(Debug, Default)]
pub struct IfdsCsrGpuScratch {
    rule_columns: IfdsCsrRuleColumns,
    rule_fingerprint: Option<IfdsCsrRuleInputFingerprint>,
    inputs: Vec<Vec<u8>>,
    static_input_key: Option<IfdsCsrStaticInputKey>,
    row_cursor: Vec<u32>,
    col_len_words: Vec<u32>,
    program_cache: ProgramCache<IfdsCsrProgramCacheKey, CachedIfdsCsrProgram>,
}

type CachedIfdsCsrProgram = CachedProgram;

impl IfdsCsrGpuScratch {
    #[cfg(test)]
    fn program_builds(&self) -> usize {
        self.program_cache.builds()
    }
}

/// Total node count of the exploded supergraph for the given
/// dimensions. Equivalent to `row_ptr.len() - 1` after the CSR is
/// built; useful when the caller needs to size frontier bitsets
/// before invoking [`reference_build_ifds_csr`].
#[must_use]
pub fn ifds_node_count(num_procs: u32, blocks_per_proc: u32, facts_per_proc: u32) -> u32 {
    ifds_node_count_saturating(num_procs, blocks_per_proc, facts_per_proc)
}

/// Helper: round-trip a dense index through the packed encoding and
/// back. Used by callers that emit findings keyed on the packed id
/// but operate on dense indices internally.
#[must_use]
pub fn round_trip_dense(dense: u32, blocks_per_proc: u32, facts_per_proc: u32) -> Option<u32> {
    let encoded = dense_to_encoded(dense, blocks_per_proc, facts_per_proc)?;
    encoded_to_dense(encoded, blocks_per_proc, facts_per_proc)
}

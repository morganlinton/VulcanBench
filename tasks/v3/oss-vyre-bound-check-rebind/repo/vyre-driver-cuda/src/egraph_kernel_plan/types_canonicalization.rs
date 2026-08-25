use super::{
    CudaEGraphCanonicalRewriteKernelResult, CudaEGraphResidentColumnSnapshot,
    CudaEGraphResidentSignatureSnapshot, CudaEGraphSignatureRefreshKernelResult,
    CudaEGraphStructuralEquivalenceKernelResult, CudaEGraphUnionCompactionPlan,
};

/// End-to-end result for one CUDA-resident e-graph structural
/// canonicalization round.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralCanonicalizationRoundResult {
    /// Exact structural duplicate discovery result.
    pub discovery: CudaEGraphStructuralEquivalenceKernelResult,
    /// Deterministic union/rewrite plan derived from discovered pairs.
    pub union_plan: CudaEGraphUnionCompactionPlan,
    /// Device-side canonical rewrite launch result.
    pub rewrite: CudaEGraphCanonicalRewriteKernelResult,
    /// Device-side row-signature refresh launch result after rewrites.
    pub signature_refresh: CudaEGraphSignatureRefreshKernelResult,
}

/// Result for iterative CUDA-resident e-graph canonicalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralCanonicalizationFixedPointResult {
    /// Rounds executed, including the final no-op round when convergence is
    /// proven before `max_rounds`.
    pub rounds: Vec<CudaEGraphStructuralCanonicalizationRoundResult>,
    /// Resident columns after the last executed round.
    pub final_snapshot: CudaEGraphResidentColumnSnapshot,
    /// `true` iff a no-op discovery round proved fixed-point convergence.
    pub converged: bool,
    /// Maximum number of rounds requested by the caller.
    pub max_rounds: usize,
    /// Total unique equivalence pairs discovered across all rounds.
    pub total_discovered_pairs: u64,
    /// Total canonical rewrite records applied across all rounds.
    pub total_rewrites: u64,
}

/// Final host readback policy for iterative CUDA-resident e-graph
/// canonicalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaEGraphFixedPointReadback {
    /// Do not copy any final resident columns back to the host. This is the
    /// hot path when the caller keeps the e-graph resident for later kernels.
    None,
    /// Copy only refreshed row signatures back to the host. This is sufficient
    /// for planning later structural candidate buckets without transferring
    /// row ids, op ids, child offsets, child lengths, or children.
    Signatures,
    /// Copy all planning columns back to the host.
    FullColumns,
}

impl Default for CudaEGraphFixedPointReadback {
    fn default() -> Self {
        Self::FullColumns
    }
}

/// Policy-aware result for iterative CUDA-resident e-graph canonicalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralCanonicalizationFixedPointReport {
    /// Rounds executed, including the final no-op round when convergence is
    /// proven before `max_rounds`.
    pub rounds: Vec<CudaEGraphStructuralCanonicalizationRoundResult>,
    /// Full resident columns after the last executed round when requested by
    /// [`CudaEGraphFixedPointReadback::FullColumns`].
    pub final_snapshot: Option<CudaEGraphResidentColumnSnapshot>,
    /// Signature-only resident snapshot after the last executed round when
    /// requested directly or derivable from a full-column readback.
    pub final_signature_snapshot: Option<CudaEGraphResidentSignatureSnapshot>,
    /// Final host readback policy used by this run.
    pub final_readback: CudaEGraphFixedPointReadback,
    /// Bytes a full final resident-column readback would transfer.
    pub final_full_readback_bytes: usize,
    /// Bytes represented by the final row-signature column.
    pub final_signature_snapshot_bytes: usize,
    /// Additional bytes transferred at the final readback boundary by the
    /// selected policy. Signature-only reports reuse the already-current
    /// planning snapshot and therefore do not force another device-to-host
    /// copy.
    pub final_additional_readback_bytes: usize,
    /// Full-column final readback bytes avoided by the selected policy.
    pub avoided_final_readback_bytes: usize,
    /// `true` iff a no-op discovery round proved fixed-point convergence.
    pub converged: bool,
    /// Maximum number of rounds requested by the caller.
    pub max_rounds: usize,
    /// Total unique equivalence pairs discovered across all rounds.
    pub total_discovered_pairs: u64,
    /// Total canonical rewrite records applied across all rounds.
    pub total_rewrites: u64,
}

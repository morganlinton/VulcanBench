use vyre_foundation::optimizer::eqsat_gpu::Equivalence;

/// CUDA pass used when applying structural e-graph equivalence output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaEGraphUnionCompactionPass {
    /// Merge canonicalized `(left, right)` e-class pairs into a device-side
    /// union-find parent column.
    UnionPairs,
    /// Rewrite non-representative e-classes to their deterministic canonical
    /// representative after path compression.
    CanonicalRewrites,
}

/// One bounded CUDA launch wave for e-graph union/compaction work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphUnionCompactionWave {
    /// Union/compaction pass.
    pub pass: CudaEGraphUnionCompactionPass,
    /// First logical pair or rewrite handled by this wave.
    pub first_item: u64,
    /// Logical pair or rewrite count handled by this wave.
    pub item_count: u64,
    /// CUDA blocks for this wave.
    pub blocks: u32,
    /// CUDA threads per block for this wave.
    pub threads_per_block: u32,
}

/// Deterministic e-class canonicalization rewrite produced after union
/// compaction planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphCanonicalRewrite {
    /// E-class id that should be rewritten.
    pub eclass_id: u32,
    /// Stable representative e-class id.
    pub representative: u32,
}

/// Deterministic apply plan for CUDA structural-equivalence output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphUnionCompactionPlan {
    /// Unique sorted non-self e-class pairs to union.
    pub canonical_pairs: Vec<Equivalence>,
    /// Sorted e-class ids touched by the union batch.
    pub affected_eclasses: Vec<u32>,
    /// Stable non-representative to representative rewrites after union
    /// closure.
    pub canonical_rewrites: Vec<CudaEGraphCanonicalRewrite>,
    /// Bounded union and rewrite launch waves.
    pub waves: Vec<CudaEGraphUnionCompactionWave>,
    /// Number of self-pairs dropped before planning.
    pub ignored_self_pair_count: u64,
    /// Number of duplicate or reversed duplicate pairs removed before
    /// planning.
    pub duplicate_pair_count: u64,
    /// Sum of logical union/rewrite items across all waves.
    pub total_items: u64,
    /// Sum of CUDA blocks across all waves.
    pub total_blocks: u64,
}

/// Device-packed canonical rewrite table consumed by the CUDA e-graph
/// canonicalization kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphCanonicalRewriteDeviceImage {
    /// Fixed-width records: `(eclass_id, representative)`.
    pub rewrite_words: Vec<u32>,
    /// Number of rewrite records.
    pub rewrite_count: usize,
    /// Number of u32 words per rewrite record.
    pub rewrite_record_words: usize,
}

/// PTX source and ABI metadata for the canonical-rewrite kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphCanonicalRewriteKernelPtx {
    /// CUDA SM target used by the PTX preamble.
    pub target_sm: u32,
    /// PTX ISA version emitted in the preamble.
    pub ptx_version: &'static str,
    /// Entry symbol resolved by the CUDA module loader.
    pub entry_name: &'static str,
    /// Number of kernel parameters in the ABI.
    pub parameter_count: usize,
    /// Number of u32 words per canonical-rewrite record.
    pub rewrite_record_words: usize,
    /// Complete PTX source.
    pub source: String,
}

/// Result produced by launching the canonical-rewrite CUDA kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphCanonicalRewriteKernelResult {
    /// Number of rewrite records uploaded for binary-search lookup.
    pub rewrite_count: usize,
    /// Number of row e-class ids covered by the launch.
    pub row_count: usize,
    /// Number of child e-class ids covered by the launch.
    pub child_count: usize,
    /// Number of CUDA launch waves issued.
    pub launch_count: usize,
    /// Sum of logical row/child items covered by launches.
    pub total_items: u64,
}

/// PTX source and ABI metadata for the row-signature refresh kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignatureRefreshKernelPtx {
    /// CUDA SM target used by the PTX preamble.
    pub target_sm: u32,
    /// PTX ISA version emitted in the preamble.
    pub ptx_version: &'static str,
    /// Entry symbol resolved by the CUDA module loader.
    pub entry_name: &'static str,
    /// Number of kernel parameters in the ABI.
    pub parameter_count: usize,
    /// Complete PTX source.
    pub source: String,
}

/// Result produced by launching the row-signature refresh CUDA kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignatureRefreshKernelResult {
    /// Number of snapshot rows covered by the launch.
    pub row_count: usize,
    /// Number of CUDA launch waves issued.
    pub launch_count: usize,
    /// Sum of logical row items covered by launches.
    pub total_rows: u64,
}

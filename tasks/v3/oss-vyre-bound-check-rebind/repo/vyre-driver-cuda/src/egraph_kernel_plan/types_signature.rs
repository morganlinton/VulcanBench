use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use vyre_foundation::optimizer::eqsat_gpu::Equivalence;

/// Rows that share one structural signature and therefore need exact
/// device-side comparison before any equivalence is emitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignatureBucket {
    /// Shared structural row signature.
    pub signature: u32,
    /// First row index for this bucket inside
    /// [`CudaEGraphSignatureBucketPlan::bucket_rows`].
    pub first_bucket_row: u32,
    /// Number of rows in this signature bucket.
    pub row_count: u32,
    /// Number of unordered row pairs represented by this bucket.
    pub candidate_pair_count: u64,
}

/// Bounded CUDA launch wave for exact comparison of signature-candidate pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignaturePairWave {
    /// Index into [`CudaEGraphSignatureBucketPlan::buckets`].
    pub bucket_index: u32,
    /// First pair ordinal inside the bucket's triangular pair space.
    pub first_pair: u64,
    /// Number of pair ordinals compared by this wave.
    pub pair_count: u64,
    /// CUDA blocks for this wave.
    pub blocks: u32,
    /// CUDA threads per block for this wave.
    pub threads_per_block: u32,
}

/// Device-work plan for structural duplicate discovery from row signatures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignatureBucketPlan {
    /// Checked resident image view used by kernels.
    pub view: CudaEGraphDeviceKernelView,
    /// Candidate buckets sorted by signature, then row index.
    pub buckets: Vec<CudaEGraphSignatureBucket>,
    /// Row ids for all buckets, concatenated by bucket.
    pub bucket_rows: Vec<u32>,
    /// Bounded pair-comparison launch waves.
    pub pair_waves: Vec<CudaEGraphSignaturePairWave>,
    /// Total exact row comparisons required after signature filtering.
    pub candidate_pair_count: u64,
    /// Sum of CUDA blocks across all pair waves.
    pub total_blocks: u64,
}

/// Exact structural-equivalence output derived from signature buckets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralEquivalencePlan {
    /// Signature bucket plan that bounded exact comparison work.
    pub signature_plan: CudaEGraphSignatureBucketPlan,
    /// Unique e-class merge candidates proven by exact packed-row comparison.
    pub equivalences: Vec<Equivalence>,
    /// Candidate pairs that survived exact row comparison.
    pub exact_pair_count: u64,
    /// Exact pairs already inside the same e-class and therefore redundant.
    pub redundant_pair_count: u64,
    /// Candidate pairs rejected after exact op/arity/child comparison.
    pub rejected_candidate_pair_count: u64,
    /// U32 words required by a compact `(left, right)` equivalence output.
    pub equivalence_output_words: usize,
}

/// Device-packed signature-bucket table consumed by structural-equivalence
/// kernels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphSignatureBucketDeviceImage {
    /// Fixed-width records: `(signature, first_bucket_row, row_count,
    /// candidate_pair_count_lo, candidate_pair_count_hi)`.
    pub bucket_words: Vec<u32>,
    /// Concatenated row ids referenced by bucket records.
    pub bucket_rows: Vec<u32>,
    /// Number of bucket records.
    pub bucket_count: usize,
    /// Number of u32 words per bucket record.
    pub bucket_record_words: usize,
    /// Total candidate pairs represented by the bucket table.
    pub candidate_pair_count: u64,
}

/// Bounded output buffers needed by a structural-equivalence kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralEquivalenceOutputPlan {
    /// Worst-case emitted equivalences before exact duplicate compaction.
    pub max_equivalences: u64,
    /// U32 words required for `(left, right)` output pairs.
    pub output_pair_words: usize,
    /// Bytes required for `(left, right)` output pairs.
    pub output_pair_bytes: usize,
    /// U32 words required for the atomic output counter.
    pub output_counter_words: usize,
    /// Bytes required for the atomic output counter.
    pub output_counter_bytes: usize,
}

/// Complete resident-kernel launch artifact for structural e-graph
/// equivalence discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralEquivalenceLaunchArtifact {
    /// Packed signature-bucket metadata uploaded beside the e-graph image.
    pub bucket_image: CudaEGraphSignatureBucketDeviceImage,
    /// Output buffer sizing for the kernel.
    pub output: CudaEGraphStructuralEquivalenceOutputPlan,
    /// Bounded pair-comparison waves to launch.
    pub pair_waves: Vec<CudaEGraphSignaturePairWave>,
}

/// PTX source and ABI metadata for the structural-equivalence kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralEquivalenceKernelPtx {
    /// CUDA SM target used by the PTX preamble.
    pub target_sm: u32,
    /// PTX ISA version emitted in the preamble.
    pub ptx_version: &'static str,
    /// Entry symbol resolved by the CUDA module loader.
    pub entry_name: &'static str,
    /// Number of kernel parameters in the ABI.
    pub parameter_count: usize,
    /// Number of u32 words per signature-bucket record.
    pub bucket_record_words: usize,
    /// Complete PTX source.
    pub source: String,
}

/// Result produced by launching the structural-equivalence CUDA kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphStructuralEquivalenceKernelResult {
    /// Raw emitted e-class pair count before host deduplication and excluding
    /// device-side overflow beyond the planned output capacity.
    pub emitted_pair_count: u64,
    /// Unique sorted e-class pairs after host compaction.
    pub unique: Vec<Equivalence>,
    /// Number of pairs reported by the device atomic counter before capping to
    /// the planned output capacity.
    pub device_reported_count: u64,
    /// Whether device output exceeded the planned output-pair capacity.
    pub overflowed_output_capacity: bool,
}

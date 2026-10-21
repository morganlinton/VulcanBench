pub(super) const DEFAULT_THREADS_PER_BLOCK: u32 = 256;
pub(super) const DEFAULT_MAX_BLOCKS_PER_LAUNCH: u32 = 65_535;
pub(super) const SIGNATURE_BUCKET_RECORD_WORDS: usize = 5;

/// CUDA structural-equivalence kernel entry symbol.
pub const CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY: &str = "main";

/// Number of u32 words in one packed signature-bucket record.
pub const CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS: usize = SIGNATURE_BUCKET_RECORD_WORDS;

/// Number of parameters in the structural-equivalence PTX kernel ABI.
pub const CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT: usize = 13;

/// CUDA canonical-rewrite kernel entry symbol.
pub const CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_ENTRY: &str = "main";

/// Number of parameters in the canonical-rewrite PTX kernel ABI.
pub const CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT: usize = 7;

/// Number of u32 words in one packed canonical-rewrite record.
pub const CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS: usize = 2;

/// CUDA row-signature refresh kernel entry symbol.
pub const CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_ENTRY: &str = "main";

/// Number of parameters in the row-signature refresh PTX kernel ABI.
pub const CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT: usize = 7;

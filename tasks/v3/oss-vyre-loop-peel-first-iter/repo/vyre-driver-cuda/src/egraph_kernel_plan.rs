//! CUDA launch-wave planning for resident e-graph device images.
//!
//! Equality-saturation kernels need deterministic row, child-edge, and
//! e-class-group work partitions. This module converts the checked resident
//! image view into bounded launch waves without rebuilding graph metadata or
//! depending on e-graph semantics in the CUDA backend.

#[path = "egraph_kernel_plan/backend_canonicalization.rs"]
mod backend_canonicalization;
#[path = "egraph_kernel_plan/backend_rewrite.rs"]
mod backend_rewrite;
#[path = "egraph_kernel_plan/backend_structural.rs"]
mod backend_structural;
#[path = "egraph_kernel_plan/constants.rs"]
mod constants;
#[path = "egraph_kernel_plan/error.rs"]
mod error;
#[path = "egraph_kernel_plan/helpers.rs"]
mod helpers;
#[path = "egraph_kernel_plan/plan_equivalence.rs"]
mod plan_equivalence;
#[path = "egraph_kernel_plan/plan_kernel_work.rs"]
mod plan_kernel_work;
#[path = "egraph_kernel_plan/plan_signature.rs"]
mod plan_signature;
#[path = "egraph_kernel_plan/plan_union.rs"]
mod plan_union;
#[path = "egraph_kernel_plan/types_canonicalization.rs"]
mod types_canonicalization;
#[path = "egraph_kernel_plan/types_launch.rs"]
mod types_launch;
#[path = "egraph_kernel_plan/types_signature.rs"]
mod types_signature;
#[path = "egraph_kernel_plan/types_snapshot.rs"]
mod types_snapshot;
#[path = "egraph_kernel_plan/types_union.rs"]
mod types_union;

#[path = "egraph_kernel_plan/args.rs"]
mod args;
#[path = "egraph_kernel_plan/ptx.rs"]
mod ptx;

#[cfg(test)]
#[path = "egraph_kernel_plan/tests/mod.rs"]
mod tests;

pub use constants::{
    CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_ENTRY, CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS, CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS,
    CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_ENTRY, CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY,
    CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT,
};
pub use error::CudaEGraphKernelPlanError;
pub use helpers::cuda_egraph_signature_pair_rows;
pub use plan_equivalence::{
    collect_cuda_egraph_structural_equivalences, pack_cuda_egraph_signature_bucket_device_image,
    plan_cuda_egraph_structural_equivalence_launch_artifact,
    plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan,
    plan_cuda_egraph_structural_equivalence_output, plan_cuda_egraph_structural_equivalences,
};
pub use plan_kernel_work::plan_cuda_egraph_kernel_work;
pub use plan_signature::{
    plan_cuda_egraph_signature_buckets, plan_cuda_egraph_signature_buckets_from_resident_snapshot,
    plan_cuda_egraph_signature_buckets_from_signature_snapshot,
};
pub use plan_union::{
    pack_cuda_egraph_canonical_rewrite_device_image, plan_cuda_egraph_union_compaction,
};
pub use ptx::{
    cuda_egraph_canonical_rewrite_kernel_ptx, cuda_egraph_signature_refresh_kernel_ptx,
    cuda_egraph_structural_equivalence_kernel_ptx,
};
pub use types_canonicalization::{
    CudaEGraphFixedPointReadback, CudaEGraphStructuralCanonicalizationFixedPointReport,
    CudaEGraphStructuralCanonicalizationFixedPointResult,
    CudaEGraphStructuralCanonicalizationRoundResult,
};
pub use types_launch::{
    CudaEGraphKernelLaunchConfig, CudaEGraphKernelPass, CudaEGraphKernelWave,
    CudaEGraphKernelWorkPlan,
};
pub use types_signature::{
    CudaEGraphSignatureBucket, CudaEGraphSignatureBucketDeviceImage, CudaEGraphSignatureBucketPlan,
    CudaEGraphSignaturePairWave, CudaEGraphStructuralEquivalenceKernelPtx,
    CudaEGraphStructuralEquivalenceKernelResult, CudaEGraphStructuralEquivalenceLaunchArtifact,
    CudaEGraphStructuralEquivalenceOutputPlan, CudaEGraphStructuralEquivalencePlan,
};
pub use types_snapshot::{CudaEGraphResidentColumnSnapshot, CudaEGraphResidentSignatureSnapshot};
pub use types_union::{
    CudaEGraphCanonicalRewrite, CudaEGraphCanonicalRewriteDeviceImage,
    CudaEGraphCanonicalRewriteKernelPtx, CudaEGraphCanonicalRewriteKernelResult,
    CudaEGraphSignatureRefreshKernelPtx, CudaEGraphSignatureRefreshKernelResult,
    CudaEGraphUnionCompactionPass, CudaEGraphUnionCompactionPlan, CudaEGraphUnionCompactionWave,
};

//! Unified entry point for the self-hosted optimizer pipeline.
//!
//! `gpu_optimize` probes the dispatcher's persistent-path support and
//! routes through the fast persistent-resident pipeline when available
//! (CUDA today), or uses the sequential per-pass GPU dispatch path
//! (canon → const-fold → pattern-match → DCE) for dispatchers that
//! only implement the borrowed `dispatch` surface (wgpu today).
//!
//! Callers don't need to know which backend they have  -  they call
//! `gpu_optimize(program, dispatcher)` and get an optimized Program
//! back through whichever path the dispatcher supports.

use vyre_foundation::ir::Program;

use super::canonicalize_via_encoded::{gpu_canonicalize, CanonicalizeError};
use super::const_fold_via_encoded::{gpu_const_fold, ConstFoldError};
use super::dce_via_encoded::{gpu_dce, DceError};
use super::dispatcher::OptimizerDispatcher;
use super::pattern_match_via_encoded::{gpu_algebraic_identities, PatternMatchError};
use super::pipeline_resident::{gpu_pipeline_resident, PipelineError};

/// Errors surfaced by `gpu_optimize`.
#[derive(Debug)]
pub enum GpuOptimizeError {
    /// Persistent-pipeline error path.
    Persistent(PipelineError),
    /// Sequential per-pass GPU dispatch errors (one variant per pass).
    Canonicalize(CanonicalizeError),
    ConstFold(ConstFoldError),
    PatternMatch(PatternMatchError),
    Dce(DceError),
}

impl std::fmt::Display for GpuOptimizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Persistent(err) => write!(f, "gpu_optimize persistent path: {err}"),
            Self::Canonicalize(err) => write!(f, "gpu_optimize canonicalize: {err}"),
            Self::ConstFold(err) => write!(f, "gpu_optimize const-fold: {err}"),
            Self::PatternMatch(err) => write!(f, "gpu_optimize pattern-match: {err}"),
            Self::Dce(err) => write!(f, "gpu_optimize dce: {err}"),
        }
    }
}

impl std::error::Error for GpuOptimizeError {}

/// Run the four-pass self-hosted optimizer on `program`. The
/// dispatcher controls whether the persistent-resident fast path or
/// the per-pass borrowed path is used.
pub fn gpu_optimize(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, GpuOptimizeError> {
    if dispatcher.supports_persistent() {
        return gpu_pipeline_resident(program, dispatcher).map_err(GpuOptimizeError::Persistent);
    }
    // Non-resident path: sequential per-pass dispatch via the borrowed
    // `dispatch` API. Each pass re-encodes; outputs flow CPU-side
    // between passes. Slower than the persistent path but works on
    // any backend that implements the basic `dispatch` method.
    let program = gpu_canonicalize(program, dispatcher).map_err(GpuOptimizeError::Canonicalize)?;
    let program = gpu_const_fold(program, dispatcher).map_err(GpuOptimizeError::ConstFold)?;
    let program =
        gpu_algebraic_identities(program, dispatcher).map_err(GpuOptimizeError::PatternMatch)?;
    let program = gpu_dce(program, dispatcher).map_err(GpuOptimizeError::Dce)?;
    Ok(program)
}

//! Linear layer: `y = x @ W + b`.
//!
//! Audit-fix A30 split this module by concern: `linear` builder + struct in
//! `mod.rs`/`builder.rs`, the tiled variants in `tiled.rs`, the fused
//! `linear_relu` in `relu.rs`, `rms_norm_linear` in `rms_norm.rs`, and tests
//! in `tests/`.

mod batch_matmul;
mod builder;
mod fused_activation;
#[cfg(feature = "nn-linear-4bit")]
mod linear_4bit;
mod relu;
mod rms_norm;
mod silu;
mod tiled;

pub use batch_matmul::batch_matmul;
pub use builder::{linear, Linear};
#[cfg(feature = "nn-linear-4bit")]
pub use linear_4bit::{
    linear_4bit, linear_4bit_affine_grouped, linear_4bit_affine_grouped_typed,
    linear_4bit_affine_grouped_planner_evidence, QuantizedLinear4BitPlannerEvidence,
    QuantizedLinear4BitSpec, LINEAR_4BIT_AFFINE_GROUPED_OUTPUT_DRIFT_ABS_TOLERANCE,
};
pub use relu::linear_relu;
pub use rms_norm::{rms_norm_linear, try_rms_norm_linear};
pub use silu::linear_silu;
pub use tiled::{linear_tiled, linear_tiled_reference};

#[cfg(test)]
mod tests;

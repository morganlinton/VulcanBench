//! Cooperative tiled matrix multiplication.
//!
//! Category-A composition. Computes `out = a @ b` where `a` is `m × k`,
//! `b` is `k × n`, `out` is `m × n`. Each workgroup owns a rectangular
//! output tile and cooperatively stages A/B k-tiles through workgroup
//! memory before accumulating one output element per lane.
//!
//! ROADMAP S10: this module was a single 960-LOC file before splitting.
//! The cuts are:
//!
//! - [`plain`]  -  `MatmulTiled` builder + `matmul_tiled` Cat-A wrapper
//!   for the no-bias variant.
//! - [`bias`]  -  `MatmulBiasTiled` builder + `matmul_bias_tiled`
//!   Cat-A wrapper for the bias-fused variant.
//! - [`shape`]  -  `MatrixShape` / `TileShape` value types and the
//!   geometry helpers (`output_tile_shape`,
//!   `padded_tile_lane_count`, `in_output_bounds`).
//! - [`body`]  -  the cooperative inner kernel body
//!   (`cooperative_matmul_body`) that both builders share.
//!
//! The public surface is the plain and bias builders/wrappers. Kernel geometry
//! and body construction stay inside this module boundary.

mod body;
mod mma_body;
mod mma_fragment;
mod ops;
mod program;
mod shape;
mod tensor_core_policy;
mod tile_coords;

pub use ops::{matmul_bias_tiled, matmul_tiled, MatmulBiasTiled, MatmulTiled};
#[cfg(feature = "nn-linear-4bit")]
pub(crate) use shape::MatrixShape;
#[cfg(feature = "nn-linear-4bit")]
pub(crate) use tensor_core_policy::{
    plan_matmul_kernel, F32MatmulMode, MatmulKernelCapabilities, MatmulKernelPath,
    MatmulFallbackReason, MatmulKernelPlan,
};

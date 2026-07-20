//! Linear algebra, scans, broadcasting, and atomic compositions.
//!
//! Every function here is a pure Category-A composition over
//! vyre-ops primitives, **except** `atomic` which is Category-B
//! (`Category::Intrinsic`) because it requires the backend to support
//! `Expr::Atomic` (F-IR-35).
//!
//! Organized into sub-dialects so each concern has its own namespace:
//! - `linalg`  -  dot, matmul, matmul_tiled
//! - `scan`  -  scan_prefix_sum
//! - `broadcast`  -  broadcast
//! - `succinct`  -  rank/select bitvector metadata
//!
//! The flat-name re-exports (`vyre_libs::math::dot`, etc.) are kept
//! for back-compat so external consumers pinning against the flat
//! surface continue to resolve.

#[cfg(feature = "math-linalg")]
pub mod linalg;

#[cfg(feature = "math-scan")]
pub mod scan;

#[cfg(feature = "math-broadcast")]
pub mod broadcast;

/// Abstract algebraic structures for dataflow, security, and scheduling.
#[cfg(feature = "math-algebra")]
pub mod algebra;

/// Succinct bitvector rank metadata.
#[cfg(feature = "math-succinct")]
pub mod succinct;

/// Atomic read-modify-write compositions (add/and/or/xor/min/max/exchange/compare_exchange)
///  -  migrated from vyre-ops per the intrinsic-vs-library rule (Expr::Atomic is an
/// existing IR variant, so these are library compositions rather than intrinsics).
pub mod atomic;
/// Average floor operation
pub mod avg_floor;
mod bit_count_ops;
mod bit_count_u32;
/// Clamp to [lo, hi] per lane (migrated from vyre-ops per the intrinsic-vs-library rule).
pub mod clamp_u32;
/// 2D convolution  -  direct 3x3 kernel base case (ROADMAP H3).
pub mod conv;
/// Fast Fourier Transform  -  base-case 4-point complex FFT (ROADMAP H2).
pub mod fft;
/// Arithmetic mean reduction
pub mod reduce_mean;
/// Welford variance reduction
pub mod reduce_variance;
/// Element-wise square operation
pub mod square;
/// Block-FMA weighted-sum reduction (ROADMAP G7).
pub mod weighted_sum;
/// Welford sum-of-squares operation
pub mod welford;
/// Wrapping negation operation
pub mod wrapping_neg;

pub(crate) mod elementwise;

pub use atomic::{
    atomic_add_u32, atomic_and_u32, atomic_compare_exchange_u32, atomic_exchange_u32,
    atomic_max_u32, atomic_min_u32, atomic_or_u32, atomic_xor_u32,
};
pub use bit_count_ops::lzcnt_u32::lzcnt_u32;
pub use bit_count_ops::tzcnt_u32::tzcnt_u32;
pub use bit_count_ops::{lzcnt_u32, tzcnt_u32};
pub use clamp_u32::clamp_u32;
pub use reduce_mean::reduce_mean;
pub use reduce_variance::reduce_variance;
pub use square::square;
pub use welford::welford_sum_of_squares;

// Flat re-exports  -  keep callers that pin against `vyre_libs::math::dot`
// (and siblings) working across the nested-tree reshape.
#[cfg(feature = "math-algebra")]
pub use algebra::{
    bool_semiring_matmul, lattice_join, lattice_meet, semiring_min_plus_mul, sketch_mix,
    try_bool_semiring_matmul, try_lattice_join, try_lattice_meet, try_semiring_min_plus_mul,
    try_sketch_mix,
};
#[cfg(feature = "math-broadcast")]
pub use broadcast::broadcast;
#[cfg(feature = "math-linalg")]
pub use linalg::{
    dot, matmul, matmul_bias, matmul_bias_tiled, matmul_tiled, Dot, Matmul, MatmulBias,
    MatmulBiasTiled, MatmulTiled,
};
#[cfg(feature = "math-scan")]
pub use scan::scan_prefix_sum;
#[cfg(feature = "math-succinct")]
pub use succinct::{
    rank1_query, rank1_superblocks, select1_query, try_rank1_query, try_rank1_superblocks,
    try_select1_query,
};

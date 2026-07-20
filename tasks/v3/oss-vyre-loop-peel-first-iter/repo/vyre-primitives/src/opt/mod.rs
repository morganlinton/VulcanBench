//! Tier 2.5 optimization primitives (#9, #14, #46).
//!
//! Convex + combinatorial optimization at GPU primitive level. Composes
//! with `vyre-primitives::math` for the underlying linear-algebra
//! kernels.

/// Homotopy continuation path-tracker step (#9). Predictor-corrector
/// step for following zeros of `H(x, t) = 0` from `t=0` (easy) to
/// `t=1` (hard). Same Program serves user combinatorial-optimization
/// dialects AND vyre's #22 megakernel scheduler ILP relaxation.
pub mod homotopy;

//! Dual CPU references for `primitive.arith.mul`.

/// Operation ID for wrapping-multiply dual references.
pub const OP_ID: &str = "primitive.arith.mul";

/// Dual-reference marker for wrapping multiplication.
pub struct MulDualReference;

define_arith_dual_reference!(
    MulDualReference,
    u32::wrapping_mul,
    super::super::common::wrapping_mul_shift_add_reference
);

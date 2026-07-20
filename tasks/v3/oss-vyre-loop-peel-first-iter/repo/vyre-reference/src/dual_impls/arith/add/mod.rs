//! Dual CPU references for `primitive.arith.add`.

/// Operation ID for wrapping-add dual references.
pub const OP_ID: &str = "primitive.arith.add";

/// Dual-reference marker for wrapping addition.
pub struct AddDualReference;

define_arith_dual_reference!(
    AddDualReference,
    u32::wrapping_add,
    super::super::common::wrapping_add_bits_reference
);

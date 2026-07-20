/// Less-than comparison dual implementation reference.
pub mod reference;

/// Operation ID for less-than-comparison dual references.
pub const OP_ID: &str = "primitive.compare.lt";

/// Dual-reference marker for unsigned less-than comparison.
pub struct LtDualReference;

define_compare_dual_reference!(
    LtDualReference,
    |left, right| left < right,
    super::super::common::lt_bytes
);

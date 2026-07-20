/// Equality-comparison dual implementation reference.
pub mod reference;

/// Operation ID for equality-comparison dual references.
pub const OP_ID: &str = "primitive.compare.eq";

/// Dual-reference marker for equality comparison.
pub struct EqDualReference;

define_compare_dual_reference!(
    EqDualReference,
    |left, right| left == right,
    super::super::common::eq_bytes
);

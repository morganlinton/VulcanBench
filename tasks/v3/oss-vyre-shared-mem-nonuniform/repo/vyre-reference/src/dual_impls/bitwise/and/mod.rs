//! Dual CPU references for `primitive.bitwise.and`.

super::common::define_binary_bitwise_dual!(
    AndDualReference,
    "primitive.bitwise.and",
    |left, right| left & right,
    |left, right| left && right
);

//! Dual CPU references for `primitive.bitwise.not`.

super::common::define_unary_bitwise_dual!(
    NotDualReference,
    "primitive.bitwise.not",
    |value| !value,
    |value| !value
);

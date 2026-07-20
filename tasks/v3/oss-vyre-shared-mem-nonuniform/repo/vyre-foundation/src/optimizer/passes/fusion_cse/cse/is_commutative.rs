//! Binary operator properties used by CSE key canonicalization.

use crate::ir::BinOp;

/// Return true when swapping operands preserves the binary operation result.
#[must_use]
#[inline]
pub fn is_commutative(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::SaturatingAdd
            | BinOp::Mul
            | BinOp::SaturatingMul
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
    )
}

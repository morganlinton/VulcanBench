//! Shared RMS expression builders for normalization and fused linear kernels.

use vyre::ir::{Expr, UnOp};

pub(crate) const EMPTY_RMS_FIX: &str =
    "Fix: rms_norm n=0 is invalid; pass at least one element or bypass normalization.";

pub(crate) fn square_expr(value: Expr) -> Expr {
    Expr::mul(value.clone(), value)
}

pub(crate) fn inverse_rms_expr(sum_sq: Expr, n: u32, eps: f32) -> Expr {
    Expr::UnOp {
        op: UnOp::InverseSqrt,
        operand: Box::new(Expr::add(
            Expr::div(sum_sq, Expr::f32(n as f32)),
            Expr::f32(eps),
        )),
    }
}

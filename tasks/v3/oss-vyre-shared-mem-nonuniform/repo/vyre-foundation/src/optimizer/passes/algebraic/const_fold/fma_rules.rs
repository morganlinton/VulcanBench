// FMA simplification rules.
//
// Covers: constant folding, identity multiplier, zero multiplier,
// zero addend elimination.

use crate::ir::eval::fold_fma_literal;
use crate::ir::Expr;

/// Fma simplifications  -  constant folding and edge-case elimination.
pub(super) fn simplify_fma(a: &Expr, b: &Expr, c: &Expr) -> Option<Expr> {
    // Full constant fold: fma(a, b, c) → a*b+c
    if let Some(folded) = fold_fma_literal(a, b, c) {
        return Some(folded);
    }
    // fma(1, b, c) → b + c   (identity multiplier)
    if matches!(a, Expr::LitF32(v) if lit_f32_eq(*v, 1.0)) {
        return Some(Expr::add(b.clone(), c.clone()));
    }
    // fma(a, 1, c) → a + c
    if matches!(b, Expr::LitF32(v) if lit_f32_eq(*v, 1.0)) {
        return Some(Expr::add(a.clone(), c.clone()));
    }
    // fma(0, finite_literal, c) → c. Do not apply when the other
    // multiplier is non-literal: `0 * NaN` and `0 * inf` must stay NaN.
    if matches!((a, b), (Expr::LitF32(v), Expr::LitF32(other)) if lit_f32_eq(*v, 0.0) && other.is_finite())
    {
        return Some(c.clone());
    }
    // fma(finite_literal, 0, c) → c under the same NaN/inf guard.
    if matches!((a, b), (Expr::LitF32(other), Expr::LitF32(v)) if other.is_finite() && lit_f32_eq(*v, 0.0))
    {
        return Some(c.clone());
    }
    None
}

#[inline]
fn lit_f32_eq(value: f32, expected: f32) -> bool {
    value.to_bits() == expected.to_bits()
}

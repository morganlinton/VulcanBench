//! Const-fold tests  -  split per audit cleanup A13 (2026-04-30) so no
//! single test file exceeds the 1000-LOC hygiene cap.

use super::super::*;
use crate::ir::{Expr, UnOp};

// ════════════════════════════════════════════════════════════
// New algebraic identity tests  -  one per rule, organized by
// submodule for contributor scale.
// ════════════════════════════════════════════════════════════

// ──── unary_rules: involutions ─────────────────────────────

#[test]
fn bitnot_bitnot_is_identity() {
    let x = Expr::var("x");
    let double_not = Expr::UnOp {
        op: UnOp::BitNot,
        operand: Box::new(Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(x.clone()),
        }),
    };
    assert_eq!(fold_expr(&double_not), Some(x));
}

// ──── unary_rules: idempotent float ops ────────────────────

#[test]
fn floor_floor_is_floor() {
    let x = Expr::var("x");
    let inner = Expr::UnOp {
        op: UnOp::Floor,
        operand: Box::new(x),
    };
    let double = Expr::UnOp {
        op: UnOp::Floor,
        operand: Box::new(inner.clone()),
    };
    assert_eq!(fold_expr(&double), Some(inner));
}
#[test]
fn ceil_ceil_is_ceil() {
    let x = Expr::var("x");
    let inner = Expr::UnOp {
        op: UnOp::Ceil,
        operand: Box::new(x),
    };
    let double = Expr::UnOp {
        op: UnOp::Ceil,
        operand: Box::new(inner.clone()),
    };
    assert_eq!(fold_expr(&double), Some(inner));
}
#[test]
fn round_round_is_round() {
    let x = Expr::var("x");
    let inner = Expr::UnOp {
        op: UnOp::Round,
        operand: Box::new(x),
    };
    let double = Expr::UnOp {
        op: UnOp::Round,
        operand: Box::new(inner.clone()),
    };
    assert_eq!(fold_expr(&double), Some(inner));
}
#[test]
fn trunc_trunc_is_trunc() {
    let x = Expr::var("x");
    let inner = Expr::UnOp {
        op: UnOp::Trunc,
        operand: Box::new(x),
    };
    let double = Expr::UnOp {
        op: UnOp::Trunc,
        operand: Box::new(inner.clone()),
    };
    assert_eq!(fold_expr(&double), Some(inner));
}
#[test]
fn sign_sign_is_sign() {
    let x = Expr::var("x");
    let inner = Expr::UnOp {
        op: UnOp::Sign,
        operand: Box::new(x),
    };
    let double = Expr::UnOp {
        op: UnOp::Sign,
        operand: Box::new(inner.clone()),
    };
    assert_eq!(fold_expr(&double), Some(inner));
}

// ──── unary_rules: trunc subsumption ───────────────────────

#[test]
fn floor_trunc_is_trunc() {
    let x = Expr::var("x");
    let t = Expr::UnOp {
        op: UnOp::Trunc,
        operand: Box::new(x),
    };
    let e = Expr::UnOp {
        op: UnOp::Floor,
        operand: Box::new(t.clone()),
    };
    assert_eq!(fold_expr(&e), Some(t));
}
#[test]
fn ceil_trunc_is_trunc() {
    let x = Expr::var("x");
    let t = Expr::UnOp {
        op: UnOp::Trunc,
        operand: Box::new(x),
    };
    let e = Expr::UnOp {
        op: UnOp::Ceil,
        operand: Box::new(t.clone()),
    };
    assert_eq!(fold_expr(&e), Some(t));
}
#[test]
fn round_trunc_is_trunc() {
    let x = Expr::var("x");
    let t = Expr::UnOp {
        op: UnOp::Trunc,
        operand: Box::new(x),
    };
    let e = Expr::UnOp {
        op: UnOp::Round,
        operand: Box::new(t.clone()),
    };
    assert_eq!(fold_expr(&e), Some(t));
}

// ──── unary_rules: trig constant fold ──────────────────────

#[test]
fn sin_zero_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Sin,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(0.0))
    );
}
#[test]
fn cos_zero_is_one() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Cos,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(1.0))
    );
}
#[test]
fn tan_zero_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Tan,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(0.0))
    );
}
#[test]
fn exp_zero_is_one() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Exp,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(1.0))
    );
}
#[test]
fn exp2_zero_is_one() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Exp2,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(1.0))
    );
}
#[test]
fn log_one_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Log,
            operand: Box::new(Expr::f32(1.0))
        }),
        Some(Expr::f32(0.0))
    );
}
#[test]
fn log2_one_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Log2,
            operand: Box::new(Expr::f32(1.0))
        }),
        Some(Expr::f32(0.0))
    );
}
#[test]
fn sqrt_one_is_one() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(Expr::f32(1.0))
        }),
        Some(Expr::f32(1.0))
    );
}
#[test]
fn sqrt_zero_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(Expr::f32(0.0))
        }),
        Some(Expr::f32(0.0))
    );
}
#[test]
fn inverse_sqrt_one_is_one() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::InverseSqrt,
            operand: Box::new(Expr::f32(1.0))
        }),
        Some(Expr::f32(1.0))
    );
}
#[test]
fn popcount_zero_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Popcount,
            operand: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(0))
    );
}
#[test]
fn clz_zero_is_32() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Clz,
            operand: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(32))
    );
}
#[test]
fn ctz_zero_is_32() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::Ctz,
            operand: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(32))
    );
}
#[test]
fn reverse_bits_zero_is_zero() {
    assert_eq!(
        fold_expr(&Expr::UnOp {
            op: UnOp::ReverseBits,
            operand: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(0))
    );
}

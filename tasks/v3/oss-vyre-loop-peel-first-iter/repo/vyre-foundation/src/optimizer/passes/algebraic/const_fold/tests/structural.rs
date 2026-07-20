//! Const-fold tests  -  split per audit cleanup A13 (2026-04-30) so no
//! single test file exceeds the 1000-LOC hygiene cap.

use super::super::*;
use super::helpers::{program_contains_literal, simple_program};
use crate::ir::{DataType, Expr, UnOp};

// ──── select_rules ─────────────────────────────────────────

#[test]
fn select_not_cond_flips() {
    let c = Expr::var("c");
    let a = Expr::var("a");
    let b = Expr::var("b");
    let nc = Expr::UnOp {
        op: UnOp::LogicalNot,
        operand: Box::new(c.clone()),
    };
    assert_eq!(
        fold_expr(&Expr::select(nc, a.clone(), b.clone())),
        Some(Expr::select(c, b, a))
    );
}

#[test]
fn select_one_zero_becomes_cast() {
    let c = Expr::var("c");
    assert_eq!(
        fold_expr(&Expr::select(c.clone(), Expr::u32(1), Expr::u32(0))),
        Some(Expr::Cast {
            target: DataType::U32,
            value: Box::new(c)
        })
    );
}

// ──── fma_rules ────────────────────────────────────────────

#[test]
fn fma_one_b_c() {
    let b = Expr::var("b");
    let c = Expr::var("c");
    assert_eq!(
        fold_expr(&Expr::fma(Expr::f32(1.0), b.clone(), c.clone())),
        Some(Expr::add(b, c))
    );
}
#[test]
fn fma_a_one_c() {
    let a = Expr::var("a");
    let c = Expr::var("c");
    assert_eq!(
        fold_expr(&Expr::fma(a.clone(), Expr::f32(1.0), c.clone())),
        Some(Expr::add(a, c))
    );
}
#[test]
fn fma_zero_b_c() {
    let c = Expr::var("c");
    assert_eq!(
        fold_expr(&Expr::fma(Expr::f32(0.0), Expr::f32(2.0), c.clone())),
        Some(c)
    );
}
#[test]
fn fma_a_zero_c() {
    let c = Expr::var("c");
    assert_eq!(
        fold_expr(&Expr::fma(Expr::f32(2.0), Expr::f32(0.0), c.clone())),
        Some(c)
    );
}
#[test]
fn fma_a_b_zero() {
    let a = Expr::var("a");
    let b = Expr::var("b");
    assert_eq!(
        fold_expr(&Expr::fma(a.clone(), b.clone(), Expr::f32(0.0))),
        None
    );
}

#[test]
fn mul_by_neg_one_folds_to_negate() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::mul(x.clone(), Expr::i32(-1))),
        Some(Expr::negate(x.clone()))
    );
    assert_eq!(
        fold_expr(&Expr::mul(Expr::f32(-1.0), x.clone())),
        Some(Expr::negate(x))
    );
}

#[test]
fn double_cast_elimination() {
    // Different-target nested casts must NOT be elided: `(u32)((i32)x)` is
    // not equal to `(u32)x` for arbitrary `x` (the i32 step changes sign
    // semantics before the widen/reinterpret). Only same-target nested
    // casts are redundant - see const_fold::cast_rules.
    let x = Expr::var("x");
    let inner_cast = Expr::Cast {
        target: crate::ir::DataType::I32,
        value: Box::new(x.clone()),
    };
    let outer_cast = Expr::Cast {
        target: crate::ir::DataType::U32,
        value: Box::new(inner_cast),
    };
    assert_eq!(fold_expr(&outer_cast), None);
}

#[test]
fn select_of_select_fusion() {
    let cond = Expr::var("c");
    let a = Expr::var("a");
    let b = Expr::var("b");
    let d = Expr::var("d");
    let inner_true = Expr::select(cond.clone(), a.clone(), b.clone());
    let outer = Expr::select(cond.clone(), inner_true, d.clone());
    assert_eq!(fold_expr(&outer), Some(Expr::select(cond, a, d)));
}

#[test]
fn select_bool_canonicalization() {
    let cond = Expr::var("c");
    let outer = Expr::select(cond.clone(), Expr::u32(0), Expr::u32(1));
    assert_eq!(
        fold_expr(&outer),
        Some(Expr::Cast {
            target: crate::ir::DataType::U32,
            value: Box::new(Expr::UnOp {
                op: UnOp::LogicalNot,
                operand: Box::new(cond),
            })
        })
    );
}

#[test]
fn reciprocal_sqrt_fusion() {
    let x = Expr::var("x");
    let sqrt = Expr::UnOp {
        op: UnOp::Sqrt,
        operand: Box::new(x.clone()),
    };
    let div = Expr::div(Expr::f32(1.0), sqrt);
    assert_eq!(
        fold_expr(&div),
        Some(Expr::UnOp {
            op: UnOp::InverseSqrt,
            operand: Box::new(x)
        })
    );
}

#[test]
fn trig_division_peephole() {
    let x = Expr::var("x");
    let sin = Expr::UnOp {
        op: UnOp::Sin,
        operand: Box::new(x.clone()),
    };
    let cos = Expr::UnOp {
        op: UnOp::Cos,
        operand: Box::new(x.clone()),
    };
    let div = Expr::div(sin, cos);
    assert_eq!(
        fold_expr(&div),
        Some(Expr::UnOp {
            op: UnOp::Tan,
            operand: Box::new(x)
        })
    );
}

#[test]
fn div_self_identity() {
    let x = Expr::var("x");
    // Should fire on integers
    assert_eq!(
        fold_expr(&Expr::div(x.clone(), x.clone())),
        Some(Expr::u32(1))
    );
    // Should NOT fire on floats
    let f = Expr::fma(Expr::var("y"), Expr::var("z"), Expr::var("w")); // known float
    assert_eq!(fold_expr(&Expr::div(f.clone(), f)), None);
}

#[test]
fn algebraic_reassociation() {
    let x = Expr::var("x");
    // (x + 3) + 4 -> x + 7
    let left = Expr::add(x.clone(), Expr::u32(3));
    let add1 = Expr::add(left, Expr::u32(4));
    assert_eq!(fold_expr(&add1), Some(Expr::add(x.clone(), Expr::u32(7))));

    // (3 + x) + 4 -> x + 7
    let left2 = Expr::add(Expr::u32(3), x.clone());
    let add2 = Expr::add(left2, Expr::u32(4));
    assert_eq!(fold_expr(&add2), Some(Expr::add(x, Expr::u32(7))));
}

#[test]
fn distributive_law_add() {
    let x = Expr::var("x");
    // (x * 3) + (x * 4) -> x * 7
    let l = Expr::mul(x.clone(), Expr::u32(3));
    let r = Expr::mul(x.clone(), Expr::u32(4));
    let add = Expr::add(l, r);
    assert_eq!(fold_expr(&add), Some(Expr::mul(x, Expr::u32(7))));
}

#[test]
fn distributive_law_sub() {
    let x = Expr::var("x");
    // (x * 5) - (x * 2) -> x * 3
    let l = Expr::mul(x.clone(), Expr::u32(5));
    let r = Expr::mul(x.clone(), Expr::u32(2));
    let sub = Expr::sub(l, r);
    assert_eq!(fold_expr(&sub), Some(Expr::mul(x, Expr::u32(3))));
}

// Numeric and bitwidth literal-fold tests.

#[test]
fn popcount_of_nonzero_literal_folds_to_count() {
    // 0xF0F0_F0F0 has 16 set bits.
    let expr = Expr::UnOp {
        op: UnOp::Popcount,
        operand: Box::new(Expr::u32(0xF0F0_F0F0)),
    };
    let folded = ConstFold::transform(simple_program(expr));
    assert!(
        program_contains_literal(&folded.program, 16),
        "popcount(0xF0F0_F0F0) must fold to literal 16; got program: {:?}",
        folded.program
    );
}

#[test]
fn clz_of_nonzero_literal_folds_to_count() {
    // 0x0000_0008 has 28 leading zeros.
    let expr = Expr::UnOp {
        op: UnOp::Clz,
        operand: Box::new(Expr::u32(0x0000_0008)),
    };
    let folded = ConstFold::transform(simple_program(expr));
    assert!(
        program_contains_literal(&folded.program, 28),
        "clz(0x00000008) must fold to literal 28"
    );
}

#[test]
fn ctz_of_nonzero_literal_folds_to_count() {
    // 0x0000_0008 has 3 trailing zeros.
    let expr = Expr::UnOp {
        op: UnOp::Ctz,
        operand: Box::new(Expr::u32(0x0000_0008)),
    };
    let folded = ConstFold::transform(simple_program(expr));
    assert!(
        program_contains_literal(&folded.program, 3),
        "ctz(0x00000008) must fold to literal 3"
    );
}

#[test]
fn reverse_bits_of_literal_folds_to_reversed() {
    // ReverseBits(1) → 0x80000000 (the high bit).
    let expr = Expr::UnOp {
        op: UnOp::ReverseBits,
        operand: Box::new(Expr::u32(1)),
    };
    let folded = ConstFold::transform(simple_program(expr));
    assert!(
        program_contains_literal(&folded.program, 0x8000_0000),
        "ReverseBits(1) must fold to 0x80000000"
    );
}

#[test]
fn bitnot_of_literal_folds_to_complement() {
    // BitNot(0) → u32::MAX.
    let expr = Expr::UnOp {
        op: UnOp::BitNot,
        operand: Box::new(Expr::u32(0)),
    };
    let folded = ConstFold::transform(simple_program(expr));
    assert!(
        program_contains_literal(&folded.program, u32::MAX),
        "BitNot(0) must fold to u32::MAX"
    );
}

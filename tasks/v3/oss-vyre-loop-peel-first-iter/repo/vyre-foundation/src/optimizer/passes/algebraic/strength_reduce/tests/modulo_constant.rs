//! Strength reduction of unsigned modulo-by-constant.
//!
//! `x % d` for a non-power-of-two `u32` constant `d` lowers to
//! `x - (x / d) * d`, reusing the Granlund-Montgomery exact division. These
//! tests prove SEMANTIC EQUIVALENCE (differential evaluation against the real
//! `%` operator), not just expression shape, and cover the boundary cases that
//! must NOT take the division path (power-of-two, 1, 0, non-literal, signed).

use super::*;

/// `x % d` over a `LitU32` divisor.
fn rem_u32(divisor: u32) -> Expr {
    Expr::BinOp {
        op: BinOp::Mod,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(divisor)),
    }
}

/// Wrapping-`u32` evaluator over exactly the node subset that the modulo
/// rewrite and Granlund-Montgomery division produce: {Var, LitU32, Add, Sub,
/// Mul, Shr, Shl, BitAnd, Mod, MulHigh}. `MulHigh` matches the crate's own
/// definition: `((a as u64) * (b as u64)) >> 32`.
fn eval_u32(expr: &Expr, x: u32) -> u32 {
    match expr {
        Expr::LitU32(v) => *v,
        Expr::LitI32(v) => *v as u32,
        Expr::Var(_) => x,
        Expr::BinOp { op, left, right } => {
            let l = eval_u32(left, x);
            let r = eval_u32(right, x);
            match op {
                BinOp::Add => l.wrapping_add(r),
                BinOp::Sub => l.wrapping_sub(r),
                BinOp::Mul => l.wrapping_mul(r),
                BinOp::Shr => l.wrapping_shr(r),
                BinOp::Shl => l.wrapping_shl(r),
                BinOp::BitAnd => l & r,
                BinOp::Mod => l % r,
                BinOp::MulHigh => ((u64::from(l)).wrapping_mul(u64::from(r)) >> 32) as u32,
                other => panic!("unexpected binop in modulo eval: {other:?}"),
            }
        }
        Expr::UnOp {
            op: UnOp::Negate,
            operand,
        } => eval_u32(operand, x).wrapping_neg(),
        other => panic!("unexpected node in modulo eval: {other:?}"),
    }
}

/// Broad input set: small values, powers of two, and the high/edge u32 range.
const FUZZ_INPUTS: [u32; 16] = [
    0,
    1,
    2,
    3,
    7,
    255,
    256,
    1000,
    65_535,
    65_536,
    1_000_000,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
    0xDEAD_BEEF,
];

/// Non-power-of-two divisors gm-division is proven to handle.
const DIVISORS: [u32; 15] = [
    3, 5, 6, 7, 9, 10, 11, 12, 15, 100, 255, 1000, 65_535, 7919, 1_000_003,
];

#[test]
fn mod_by_seven_lowers_to_division_back_multiply() {
    // Shape contract: x % 7 → x - (x / 7) * 7.
    let reduced = reduce_expr(&rem_u32(7)).expect("Fix: x % 7 must strength-reduce");
    match &reduced {
        Expr::BinOp {
            op: BinOp::Sub,
            left,
            right,
        } => {
            assert!(
                matches!(left.as_ref(), Expr::Var(_)),
                "minuend must be the dividend x: {left:?}"
            );
            match right.as_ref() {
                Expr::BinOp {
                    op: BinOp::Mul,
                    right: divisor,
                    ..
                } => assert!(
                    matches!(divisor.as_ref(), Expr::LitU32(7)),
                    "subtrahend must back-multiply by the divisor 7: {divisor:?}"
                ),
                other => panic!("expected (quotient * 7), got {other:?}"),
            }
        }
        other => panic!("expected x - (x / 7) * 7, got {other:?}"),
    }
}

#[test]
fn mod_by_constant_is_exact_under_wrapping() {
    // Differential truth: the rewritten tree evaluates to the same value as
    // the real `%` operator for every (divisor, input) pair, including the
    // divisor-boundary values where floor(x/d) ticks over.
    for &d in &DIVISORS {
        let reduced =
            reduce_expr(&rem_u32(d)).unwrap_or_else(|| panic!("Fix: x % {d} must strength-reduce"));
        for &x in &FUZZ_INPUTS {
            assert_eq!(eval_u32(&reduced, x), x % d, "x={x} d={d}");
        }
        for x in [
            d.wrapping_sub(1),
            d,
            d.wrapping_add(1),
            d.wrapping_mul(2).wrapping_sub(1),
            d.wrapping_mul(2),
        ] {
            assert_eq!(eval_u32(&reduced, x), x % d, "boundary x={x} d={d}");
        }
    }
}

#[test]
fn mod_by_power_of_two_uses_bitand_not_division() {
    // Adversarial: power-of-two divisors must stay on the cheap AND-mask path,
    // never the division back-multiply.
    for d in [2u32, 4, 8, 16, 1024, 0x8000_0000] {
        let reduced =
            reduce_expr(&rem_u32(d)).unwrap_or_else(|| panic!("x % {d} (2^k) must reduce"));
        match &reduced {
            Expr::BinOp {
                op: BinOp::BitAnd,
                right,
                ..
            } => assert!(
                matches!(right.as_ref(), Expr::LitU32(m) if *m == d - 1),
                "x % {d} must be x & {}: {right:?}",
                d - 1
            ),
            other => panic!("x % {d} must be a BitAnd, got {other:?}"),
        }
        for &x in &FUZZ_INPUTS {
            assert_eq!(eval_u32(&reduced, x), x % d, "2^k exactness x={x} d={d}");
        }
    }
}

#[test]
fn mod_by_one_folds_to_zero() {
    assert!(
        matches!(reduce_expr(&rem_u32(1)), Some(Expr::LitU32(0))),
        "x % 1 must fold to 0"
    );
}

#[test]
fn mod_by_zero_is_left_intact() {
    // Modulo-by-zero is a backend trap, never a rewrite. Must not panic and
    // must not produce a rewritten tree.
    assert!(
        reduce_expr(&rem_u32(0)).is_none(),
        "x % 0 must not be rewritten"
    );
}

#[test]
fn mod_by_non_literal_divisor_is_left_intact() {
    let dynamic = Expr::BinOp {
        op: BinOp::Mod,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::var("x")),
    };
    assert!(
        reduce_expr(&dynamic).is_none(),
        "x % y (non-literal divisor) must not be rewritten"
    );
}

#[test]
fn mod_by_signed_literal_is_left_intact() {
    // Signed divisor literal: the unsigned strength reduction must not fire
    // (signed remainder rounds toward zero, not floor).
    let signed = Expr::BinOp {
        op: BinOp::Mod,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::i32(7)),
    };
    assert!(
        reduce_expr(&signed).is_none(),
        "x % 7i32 must not take the unsigned division path"
    );
}

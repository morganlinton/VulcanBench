//! Direct differential verification of strength-reduced unsigned
//! division-by-constant (Granlund-Montgomery).
//!
//! `modulo_constant.rs` proves `x % d` exact, which exercises the GM
//! division only TRANSITIVELY via `x - (x / d) * d`. This file proves the
//! division ITSELF, byte-for-byte against the real `/` operator, directly
//! and over a wide random input space — most importantly across the
//! `needs_fixup` divisors whose `(t + ((n - t) >> 1)) >> (s - 1)` sequence
//! is the subtle one (an off-by-one there is invisible to shape tests but
//! caught by a differential).

use super::modulo_constant::{eval_u32, DIVISORS, FUZZ_INPUTS};
use super::*;
use proptest::prelude::*;

/// `x / d` over a `LitU32` divisor.
fn div_expr(divisor: u32) -> Expr {
    Expr::div(Expr::var("x"), Expr::u32(divisor))
}

/// True if `expr` contains an `Add` node — the structural marker of the GM
/// `needs_fixup` sequence (`t + ((n - t) >> 1)`). The non-fixup form is a
/// plain `Shr(MulHigh(..), s)` with no addition, so this distinguishes the
/// two emission paths.
fn contains_add(expr: &Expr) -> bool {
    match expr {
        Expr::BinOp { op: BinOp::Add, .. } => true,
        Expr::BinOp { left, right, .. } => contains_add(left) || contains_add(right),
        Expr::UnOp { operand, .. } => contains_add(operand),
        _ => false,
    }
}

#[test]
fn div_reduction_exercises_both_fixup_and_nonfixup_paths() {
    // d = 3 takes the simple path: `mulhi(x, M) >> s`, no Add.
    let three = reduce_expr(&div_expr(3)).expect("x / 3 must strength-reduce");
    assert!(
        !contains_add(&three),
        "x / 3 is the non-fixup path (Shr(MulHigh)), must contain no Add: {three:?}"
    );
    // d = 7 takes the fixup path: `(t + ((x - t) >> 1)) >> (s - 1)`.
    let seven = reduce_expr(&div_expr(7)).expect("x / 7 must strength-reduce");
    assert!(
        contains_add(&seven),
        "x / 7 is the fixup path, must contain the `t + ...` Add: {seven:?}"
    );
}

#[test]
fn div_by_constant_is_exact_over_fuzz_and_boundaries() {
    // Differential truth: the rewritten division tree evaluates to the same
    // value as the real `/` operator for every (divisor, input) pair,
    // including the divisor-boundary values where floor(x/d) ticks over and
    // the top of the u32 range (where the fixup add can overflow if wrong).
    for &d in &DIVISORS {
        let reduced =
            reduce_expr(&div_expr(d)).unwrap_or_else(|| panic!("Fix: x / {d} must strength-reduce"));
        for &x in &FUZZ_INPUTS {
            assert_eq!(eval_u32(&reduced, x), x / d, "x={x} d={d}");
        }
        for x in [
            d.wrapping_sub(1),
            d,
            d.wrapping_add(1),
            d.wrapping_mul(2).wrapping_sub(1),
            d.wrapping_mul(2),
            u32::MAX - d,
            u32::MAX,
        ] {
            assert_eq!(eval_u32(&reduced, x), x / d, "boundary x={x} d={d}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Differential truth over a wide random input space: the reduced
    /// division tree evaluates identically to the real `/` operator for
    /// every (non-power-of-two divisor, dividend) pair. 20k cases sweep
    /// both the fixup and non-fixup emission paths across the full u32
    /// dividend range.
    #[test]
    fn div_by_constant_exact_under_proptest(x in any::<u32>(), d in 3u32..=u32::MAX) {
        prop_assume!(!d.is_power_of_two());
        let reduced =
            reduce_expr(&div_expr(d)).expect("non-power-of-two divisor must strength-reduce");
        prop_assert_eq!(eval_u32(&reduced, x), x / d);
    }
}

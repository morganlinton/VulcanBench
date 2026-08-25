//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn shift_by_zero_is_identity() {
    let result = reduce_expr(&Expr::shl(Expr::var("x"), Expr::u32(0)));
    assert_eq!(result, Some(Expr::var("x")));
}

#[test]
fn shr_by_zero_is_identity() {
    let result = reduce_expr(&Expr::shr(Expr::var("x"), Expr::u32(0)));
    assert_eq!(result, Some(Expr::var("x")));
}

#[test]
fn chained_shl_fuses() {
    // (x << 3) << 4 → x << 7
    let inner = Expr::shl(Expr::var("x"), Expr::u32(3));
    let result = reduce_expr(&Expr::shl(inner, Expr::u32(4)));
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        matches!(
            &r,
            Expr::BinOp {
                op: BinOp::Shl,
                right,
                ..
            } if matches!(right.as_ref(), Expr::LitU32(7))
        ),
        "must fuse to x<<7: {r:?}"
    );
}

#[test]
fn chained_shr_fuses() {
    // (x >> 2) >> 5 → x >> 7
    let inner = Expr::shr(Expr::var("x"), Expr::u32(2));
    let result = reduce_expr(&Expr::shr(inner, Expr::u32(5)));
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        matches!(
            &r,
            Expr::BinOp {
                op: BinOp::Shr,
                right,
                ..
            } if matches!(right.as_ref(), Expr::LitU32(7))
        ),
        "must fuse to x>>7: {r:?}"
    );
}

#[test]
fn mixed_shift_does_not_fuse() {
    // (x << 3) >> 4 must NOT fuse (different directions)
    let inner = Expr::shl(Expr::var("x"), Expr::u32(3));
    let result = reduce_expr(&Expr::shr(inner, Expr::u32(4)));
    assert!(result.is_none(), "mixed-direction shifts must not fuse");
}

// ── Negation fusion tests ────────────────────────────────────────

#[test]
fn add_neg_becomes_sub() {
    // x + (-y) → x - y
    let result = reduce_expr(&Expr::add(Expr::var("x"), Expr::negate(Expr::var("y"))));
    let expected = Expr::sub(Expr::var("x"), Expr::var("y"));
    assert_eq!(result, Some(expected));
}

#[test]
fn neg_add_becomes_sub() {
    // (-x) + y → y - x
    let result = reduce_expr(&Expr::add(Expr::negate(Expr::var("x")), Expr::var("y")));
    let expected = Expr::sub(Expr::var("y"), Expr::var("x"));
    assert_eq!(result, Some(expected));
}

#[test]
fn sub_neg_becomes_add() {
    // x - (-y) → x + y
    let result = reduce_expr(&Expr::sub(Expr::var("x"), Expr::negate(Expr::var("y"))));
    let expected = Expr::add(Expr::var("x"), Expr::var("y"));
    assert_eq!(result, Some(expected));
}

#[test]
fn reverse_float_mul_sub_becomes_fma() {
    // c - (a * b) -> fma(-a, b, c)
    let result = reduce_expr(&Expr::sub(
        Expr::f32(1.0),
        Expr::mul(Expr::var("a"), Expr::var("b")),
    ));
    let reduced = result.expect("Fix: reverse multiply-subtract must synthesize FMA");
    assert!(
        matches!(
            &reduced,
            Expr::Fma {
                a,
                b,
                c
            } if matches!(a.as_ref(), Expr::UnOp { op: UnOp::Negate, .. })
                && matches!(b.as_ref(), Expr::Var(name) if name == "b")
                && matches!(c.as_ref(), Expr::LitF32(v) if *v == 1.0)
        ),
        "expected fma(-a, b, c), got {reduced:?}"
    );
}

// ════════════════════════════════════════════════════════════
// New strength reduction tests  -  complement laws, rotate,
// AbsDiff, Min/Max bounds, comparison strength reduction.
// ════════════════════════════════════════════════════════════

// ──── Complement annihilator / all-ones ────────────────────

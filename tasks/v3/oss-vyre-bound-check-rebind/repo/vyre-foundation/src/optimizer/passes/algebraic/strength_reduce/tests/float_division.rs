//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn float_mul_by_two_becomes_add() {
    // x * 2.0 → x + x
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::f32(2.0)));
    assert!(result.is_some());
    let reduced = result.unwrap();
    assert!(matches!(&reduced, Expr::BinOp { op: BinOp::Add, .. }));
}

#[test]
fn float_mul_by_one_becomes_identity() {
    // x * 1.0 → x
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::f32(1.0)));
    assert_eq!(result, Some(Expr::var("x")));
}

#[test]
fn float_mul_by_zero_does_not_hide_runtime_nan() {
    let result = reduce_expr(&Expr::mul(Expr::var("x"), Expr::f32(0.0)));
    assert_eq!(result, None);
}

#[test]
fn float_div_by_two_becomes_mul_half() {
    // x / 2.0 → x * 0.5
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::f32(2.0)));
    assert!(result.is_some());
    let reduced = result.unwrap();
    assert!(matches!(&reduced, Expr::BinOp { op: BinOp::Mul, .. }));
}

#[test]
fn float_add_zero_becomes_identity() {
    // x + 0.0 → x
    let result = reduce_expr(&Expr::add(Expr::var("x"), Expr::f32(0.0)));
    assert_eq!(result, Some(Expr::var("x")));
}

#[test]
fn float_sub_zero_becomes_identity() {
    // x - 0.0 → x
    let result = reduce_expr(&Expr::sub(Expr::var("x"), Expr::f32(0.0)));
    assert_eq!(result, Some(Expr::var("x")));
}

#[test]
fn int_div_by_power_of_two_becomes_shr() {
    // x / 8 → x >> 3
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::u32(8)));
    assert!(result.is_some());
    let reduced = result.unwrap();
    assert!(matches!(&reduced, Expr::BinOp { op: BinOp::Shr, .. }));
}

#[test]
fn int_div_by_constant_becomes_mulhi() {
    // x / 3 → mulhi(x, magic) >> shift (Granlund-Montgomery)
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::u32(3)));
    assert!(result.is_some(), "x/3 must be strength-reduced");
    let reduced = result.unwrap();
    // The top-level should be a Shr wrapping a MulHigh.
    match &reduced {
        Expr::BinOp {
            op: BinOp::Shr,
            left,
            ..
        } => {
            assert!(
                matches!(
                    left.as_ref(),
                    Expr::BinOp {
                        op: BinOp::MulHigh,
                        ..
                    }
                ),
                "inner must be MulHigh: {left:?}"
            );
        }
        other => panic!("x/3 must reduce to Shr(MulHigh(...)), got {other:?}"),
    }
}

#[test]
fn int_div_by_seven_uses_fixup() {
    // x / 7 needs the fixup path: (t + ((x - t) >> 1)) >> s
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::u32(7)));
    assert!(result.is_some(), "x/7 must be strength-reduced");
    // Top level should be Shr wrapping an Add (the fixup accumulation)
    let reduced = result.unwrap();
    match &reduced {
        Expr::BinOp {
            op: BinOp::Shr,
            left,
            ..
        } => {
            assert!(
                matches!(left.as_ref(), Expr::BinOp { op: BinOp::Add, .. }),
                "fixup must produce Add at top: {left:?}"
            );
        }
        other => panic!("x/7 must reduce to Shr(Add(...)), got {other:?}"),
    }
}

#[test]
fn int_mod_by_power_of_two_becomes_bitand() {
    // x % 16 → x & 15
    let result = reduce_expr(&Expr::BinOp {
        op: BinOp::Mod,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(16)),
    });
    assert!(result.is_some());
    let reduced = result.unwrap();
    assert!(matches!(
        &reduced,
        Expr::BinOp {
            op: BinOp::BitAnd,
            ..
        }
    ));
}

#[test]
fn float_div_by_constant_becomes_reciprocal_mul() {
    // x / 3.0 → x * (1.0/3.0)
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::f32(3.0)));
    assert!(result.is_some());
    let reduced = result.unwrap();
    match &reduced {
        Expr::BinOp {
            op: BinOp::Mul,
            right,
            ..
        } => match right.as_ref() {
            Expr::LitF32(v) => {
                assert!((v - 1.0 / 3.0).abs() < 1e-7, "reciprocal should be ~0.333");
            }
            other => panic!("expected LitF32 reciprocal, got {other:?}"),
        },
        other => panic!("expected Mul, got {other:?}"),
    }
}

#[test]
fn float_one_div_variable_becomes_reciprocal_unop() {
    let result = reduce_expr(&Expr::div(Expr::f32(1.0), Expr::var("x")));
    assert_eq!(result, Some(Expr::reciprocal(Expr::var("x"))));
}

#[test]
fn float_div_by_nan_does_not_reduce() {
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::f32(f32::NAN)));
    assert!(result.is_none(), "NaN divisor must not fold");
}

#[test]
fn float_div_by_zero_does_not_reduce() {
    let result = reduce_expr(&Expr::div(Expr::var("x"), Expr::f32(0.0)));
    assert!(result.is_none(), "zero divisor must not fold");
}

// ── Shift-add decomposition tests ────────────────────────────────

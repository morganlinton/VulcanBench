//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn div_one_by_constant_folds_to_reciprocal_literal() {
    // 1.0 / 4.0 → LitF32(0.25)
    let result = reduce_expr(&Expr::div(Expr::f32(1.0), Expr::f32(4.0)));
    assert_eq!(
        result,
        Some(Expr::f32(0.25)),
        "Div(1.0, 4.0) must fold to LitF32(0.25)"
    );
}

#[test]
fn div_one_by_zero_does_not_fold() {
    // 1.0 / 0.0 → stays as-is (div-by-zero is the IR's defined trap path)
    let result = reduce_expr(&Expr::div(Expr::f32(1.0), Expr::f32(0.0)));
    assert!(
        result.is_none(),
        "Div(1.0, 0.0) must NOT fold  -  div-by-zero is a trap"
    );
}

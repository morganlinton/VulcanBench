//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn negate_negate_cancels_to_identity() {
    let x = Expr::var("x");
    let expr = Expr::UnOp {
        op: crate::ir::UnOp::Negate,
        operand: Box::new(Expr::UnOp {
            op: crate::ir::UnOp::Negate,
            operand: Box::new(x.clone()),
        }),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(x),
        "Negate(Negate(x)) must reduce to x; without this every double-negation chain pays \
         two extra ops at runtime"
    );
}

#[test]
fn bitnot_bitnot_cancels_to_identity() {
    let x = Expr::var("x");
    let expr = Expr::UnOp {
        op: crate::ir::UnOp::BitNot,
        operand: Box::new(Expr::UnOp {
            op: crate::ir::UnOp::BitNot,
            operand: Box::new(x.clone()),
        }),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(x),
        "BitNot(BitNot(x)) must reduce to x"
    );
}

#[test]
fn reverse_bits_self_inverse() {
    let x = Expr::var("x");
    let expr = Expr::UnOp {
        op: crate::ir::UnOp::ReverseBits,
        operand: Box::new(Expr::UnOp {
            op: crate::ir::UnOp::ReverseBits,
            operand: Box::new(x.clone()),
        }),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(x),
        "ReverseBits(ReverseBits(x)) must reduce to x"
    );
}

#[test]
fn select_with_identical_arms_collapses_to_arm() {
    let x = Expr::var("x");
    let cond = Expr::var("c");
    let expr = Expr::Select {
        cond: Box::new(cond),
        true_val: Box::new(x.clone()),
        false_val: Box::new(x.clone()),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(x),
        "select(c, x, x) must collapse to x  -  the condition is dead. Without this, \
         post-CSE merges that collapse both arms still pay the branch."
    );
}

#[test]
fn select_with_constant_true_collapses_to_true_arm() {
    let true_arm = Expr::u32(42);
    let false_arm = Expr::u32(99);
    let expr = Expr::Select {
        cond: Box::new(Expr::bool(true)),
        true_val: Box::new(true_arm.clone()),
        false_val: Box::new(false_arm),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(true_arm),
        "select(true, a, b) must collapse to a"
    );
}

#[test]
fn select_with_constant_false_collapses_to_false_arm() {
    let true_arm = Expr::u32(42);
    let false_arm = Expr::u32(99);
    let expr = Expr::Select {
        cond: Box::new(Expr::bool(false)),
        true_val: Box::new(true_arm),
        false_val: Box::new(false_arm.clone()),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        Some(false_arm),
        "select(false, a, b) must collapse to b"
    );
}

#[test]
fn negate_single_does_not_collapse() {
    // Negate of a non-Negate operand must NOT match  -  only the
    // Negate-of-Negate self-inverse pattern fires.
    let x = Expr::var("x");
    let expr = Expr::UnOp {
        op: crate::ir::UnOp::Negate,
        operand: Box::new(x),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        None,
        "Negate(x) on its own must not be rewritten  -  the peephole only fires on \
         Negate(Negate(x))"
    );
}

#[test]
fn select_with_distinct_arms_does_not_collapse() {
    let x = Expr::var("x");
    let y = Expr::var("y");
    let cond = Expr::var("c");
    let expr = Expr::Select {
        cond: Box::new(cond),
        true_val: Box::new(x),
        false_val: Box::new(y),
    };
    assert_eq!(
        crate::optimizer::passes::algebraic::const_fold::fold_expr(&expr),
        None,
        "select(c, x, y) with distinct arms must NOT collapse  -  without this contract a \
         legitimate branch would be silently rewritten away"
    );
}

// ── Task 4 / ROADMAP G2: reciprocal constant-fold ────────────────

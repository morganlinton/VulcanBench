//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn bitand_complement_is_zero() {
    // x & ~x → 0
    let x = Expr::var("x");
    let expr = Expr::bitand(
        x.clone(),
        Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(x),
        },
    );
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn bitand_complement_reversed() {
    // ~x & x → 0
    let x = Expr::var("x");
    let expr = Expr::bitand(
        Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(x.clone()),
        },
        x,
    );
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn bitor_complement_is_all_ones() {
    // x | ~x → 0xFFFFFFFF
    let x = Expr::var("x");
    let expr = Expr::bitor(
        x.clone(),
        Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(x),
        },
    );
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(u32::MAX)));
}

#[test]
fn bitxor_complement_is_all_ones() {
    // x ^ ~x → 0xFFFFFFFF
    let x = Expr::var("x");
    let expr = Expr::bitxor(
        x.clone(),
        Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(x),
        },
    );
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(u32::MAX)));
}

// ──── Rotate identity ─────────────────────────────────────

#[test]
fn rotate_left_zero_is_identity() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::RotateLeft,
        left: Box::new(x.clone()),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(x));
}

#[test]
fn rotate_right_zero_is_identity() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::RotateRight,
        left: Box::new(x.clone()),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(x));
}

#[test]
fn rotate_left_32_is_identity() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::RotateLeft,
        left: Box::new(x.clone()),
        right: Box::new(Expr::u32(32)),
    };
    assert_eq!(reduce_expr(&expr), Some(x));
}

// ──── AbsDiff self ─────────────────────────────────────────

#[test]
fn absdiff_self_is_zero() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::AbsDiff,
        left: Box::new(x.clone()),
        right: Box::new(x),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(0)));
}

// ──── Min/Max with literal extremes ───────────────────────

#[test]
fn min_zero_unsigned_is_zero() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Min,
        left: Box::new(x),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(0)));
}

#[test]
fn max_zero_unsigned_is_x() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Max,
        left: Box::new(x.clone()),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(x));
}

#[test]
fn min_max_unsigned_is_x() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Min,
        left: Box::new(x.clone()),
        right: Box::new(Expr::u32(u32::MAX)),
    };
    assert_eq!(reduce_expr(&expr), Some(x));
}

#[test]
fn max_max_unsigned_is_max() {
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Max,
        left: Box::new(x),
        right: Box::new(Expr::u32(u32::MAX)),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::u32(u32::MAX)));
}

// ──── Unsigned comparison strength reduction ──────────────

#[test]
fn lt_zero_unsigned_is_false() {
    // x < 0 is always false for u32
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Lt,
        left: Box::new(x),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::bool(false)));
}

#[test]
fn ge_zero_unsigned_is_true() {
    // x >= 0 is always true for u32
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Ge,
        left: Box::new(x),
        right: Box::new(Expr::u32(0)),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::bool(true)));
}

#[test]
fn zero_gt_x_unsigned_is_false() {
    // 0 > x is always false for u32
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Gt,
        left: Box::new(Expr::u32(0)),
        right: Box::new(x),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::bool(false)));
}

#[test]
fn zero_le_x_unsigned_is_true() {
    // 0 <= x is always true for u32
    let x = Expr::var("x");
    let expr = Expr::BinOp {
        op: BinOp::Le,
        left: Box::new(Expr::u32(0)),
        right: Box::new(x),
    };
    assert_eq!(reduce_expr(&expr), Some(Expr::bool(true)));
}

// UnOp self-inverse and Select identities.

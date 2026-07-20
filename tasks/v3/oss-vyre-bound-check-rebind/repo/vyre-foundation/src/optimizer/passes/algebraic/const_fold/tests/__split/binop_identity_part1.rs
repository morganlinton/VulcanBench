use super::*;

#[test]
fn eq_self_is_true() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Eq,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(true))
    );
}
#[test]
fn ne_self_is_false() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ne,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(false))
    );
}
#[test]
fn lt_self_is_false() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Lt,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(false))
    );
}
#[test]
fn gt_self_is_false() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Gt,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(false))
    );
}
#[test]
fn le_self_is_true() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Le,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(true))
    );
}
#[test]
fn ge_self_is_true() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ge,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::bool(true))
    );
}

// ──── binop_identities: mod/min/max/div ────────────────────

#[test]
fn mod_one_is_zero() {
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Mod,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::u32(1))
        }),
        Some(Expr::u32(0))
    );
}
#[test]
fn mod_self_is_zero() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Mod,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::u32(0))
    );
}
#[test]
fn min_self_is_self() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Min,
            left: Box::new(x.clone()),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}
#[test]
fn max_self_is_self() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Max,
            left: Box::new(x.clone()),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}
#[test]
fn div_self_is_one() {
    let x = Expr::var("x");
    assert_eq!(fold_expr(&Expr::div(x.clone(), x)), Some(Expr::u32(1)));
}

// ──── binop_identities: wrapping/saturating ────────────────

#[test]
fn wrapping_add_zero() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::WrappingAdd,
            left: Box::new(x.clone()),
            right: Box::new(Expr::u32(0))
        }),
        Some(x)
    );
}
#[test]
fn wrapping_sub_self() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::WrappingSub,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::u32(0))
    );
}
#[test]
fn saturating_add_zero() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::SaturatingAdd,
            left: Box::new(x.clone()),
            right: Box::new(Expr::u32(0))
        }),
        Some(x)
    );
}
#[test]
fn saturating_sub_self() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::SaturatingSub,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        Some(Expr::u32(0))
    );
}
#[test]
fn saturating_mul_one() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::SaturatingMul,
            left: Box::new(x.clone()),
            right: Box::new(Expr::u32(1))
        }),
        Some(x)
    );
}
#[test]
fn saturating_mul_zero() {
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::SaturatingMul,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(0))
    );
}

// ──── binop_identities: logical boolean ────────────────────

#[test]
fn and_true_id() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::And,
            left: Box::new(Expr::bool(true)),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}
#[test]
fn and_false_ann() {
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::And,
            left: Box::new(Expr::bool(false)),
            right: Box::new(Expr::var("x"))
        }),
        Some(Expr::bool(false))
    );
}
#[test]
fn or_true_ann() {
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Or,
            left: Box::new(Expr::bool(true)),
            right: Box::new(Expr::var("x"))
        }),
        Some(Expr::bool(true))
    );
}
#[test]
fn or_false_id() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Or,
            left: Box::new(Expr::bool(false)),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}

// ──── binop_identities: all-ones mask ──────────────────────

#[test]
fn bitand_all_ones() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::bitand(x.clone(), Expr::u32(u32::MAX))),
        Some(x)
    );
}
#[test]
fn bitor_all_ones() {
    assert_eq!(
        fold_expr(&Expr::bitor(Expr::var("x"), Expr::u32(u32::MAX))),
        Some(Expr::u32(u32::MAX))
    );
}

// ──── ROADMAP A25: chained-predicate boolean simplification ─────────

#[test]
fn and_x_not_x_is_false_contradiction() {
    let x = Expr::var("c");
    let not_x = Expr::UnOp {
        op: crate::ir::UnOp::LogicalNot,
        operand: Box::new(x.clone()),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::And,
            left: Box::new(x),
            right: Box::new(not_x)
        }),
        Some(Expr::bool(false))
    );
}

#[test]
fn and_not_x_x_is_false_contradiction_left_not() {
    let x = Expr::var("c");
    let not_x = Expr::UnOp {
        op: crate::ir::UnOp::LogicalNot,
        operand: Box::new(x.clone()),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::And,
            left: Box::new(not_x),
            right: Box::new(x)
        }),
        Some(Expr::bool(false))
    );
}

#[test]
fn or_x_not_x_is_true_tautology() {
    let x = Expr::var("c");
    let not_x = Expr::UnOp {
        op: crate::ir::UnOp::LogicalNot,
        operand: Box::new(x.clone()),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Or,
            left: Box::new(x),
            right: Box::new(not_x)
        }),
        Some(Expr::bool(true))
    );
}

#[test]
fn or_not_x_x_is_true_tautology_left_not() {
    let x = Expr::var("c");
    let not_x = Expr::UnOp {
        op: crate::ir::UnOp::LogicalNot,
        operand: Box::new(x.clone()),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Or,
            left: Box::new(not_x),
            right: Box::new(x)
        }),
        Some(Expr::bool(true))
    );
}

#[test]
fn absorption_and_over_or() {
    let x = Expr::var("x");
    let y = Expr::var("y");
    let or_xy = Expr::BinOp {
        op: crate::ir::BinOp::Or,
        left: Box::new(x.clone()),
        right: Box::new(y),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::And,
            left: Box::new(x.clone()),
            right: Box::new(or_xy)
        }),
        Some(x)
    );
}

#[test]
fn absorption_or_over_and() {
    let x = Expr::var("x");
    let y = Expr::var("y");
    let and_xy = Expr::BinOp {
        op: crate::ir::BinOp::And,
        left: Box::new(x.clone()),
        right: Box::new(y),
    };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Or,
            left: Box::new(x.clone()),
            right: Box::new(and_xy)
        }),
        Some(x)
    );
}

#[test]
fn reflexive_eq_on_load_does_not_fold() {
    // Adversarial: Eq(Load, Load) MUST NOT fold  -  repeated Loads can
    // observe distinct memory under relaxed ordering. The
    // is_simple_pure guard rejects Loads.
    let load = Expr::load("buf", Expr::u32(0));
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Eq,
            left: Box::new(load.clone()),
            right: Box::new(load)
        }),
        None,
        "Eq(Load, Load) must not fold"
    );
}

// ──── ROADMAP A35: range-based fold identities ──────────────────────

#[test]
fn min_with_u32_max_is_identity() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Min,
            left: Box::new(x.clone()),
            right: Box::new(Expr::u32(u32::MAX))
        }),
        Some(x.clone())
    );
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Min,
            left: Box::new(Expr::u32(u32::MAX)),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}


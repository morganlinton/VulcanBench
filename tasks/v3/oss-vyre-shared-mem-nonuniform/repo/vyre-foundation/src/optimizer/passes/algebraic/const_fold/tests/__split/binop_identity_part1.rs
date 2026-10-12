use super::*;

// ──── Reflexive comparison folds: `Var` must NOT fold (float-NaN) ────
//
// const_fold is type-blind: a bare `Var` may bind a float that is `NaN`
// at runtime, where `x == x` is *false* and `x != x` is *true* under
// IEEE-754 (the `vyre-reference::binop_f32` oracle and the SPIR-V
// `OpFOrdEqual` emitter both honor this). Folding `Var cmp Var` to a
// bool literal type-blind miscompiles the canonical hand-rolled NaN
// check, so the `is_reflexive_cmp_safe` guard rejects `Var`. These six
// tests pin the decline; before the guard was tightened they asserted
// the (unsound) folded bool literal.

#[test]
fn eq_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Eq,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Eq(Var, Var) must not fold: x may be float NaN where x == x is false"
    );
}
#[test]
fn ne_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ne,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Ne(Var, Var) must not fold: x may be float NaN where x != x is true"
    );
}
#[test]
fn lt_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Lt,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Lt(Var, Var) must not fold: float NaN comparisons are all false"
    );
}
#[test]
fn gt_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Gt,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Gt(Var, Var) must not fold: float NaN comparisons are all false"
    );
}
#[test]
fn le_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Le,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Le(Var, Var) must not fold: NaN <= NaN is false, not true"
    );
}
#[test]
fn ge_self_var_does_not_fold() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ge,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Ge(Var, Var) must not fold: NaN >= NaN is false, not true"
    );
}

// ──── Reflexive comparison folds: provably-u32 builtins DO fold ────
//
// `InvocationId` is a deterministic u32 lane index that can never be a
// float NaN, so reflexive comparison folding stays sound and active for
// it. These prove the sound path is preserved (not blanket-disabled).

#[test]
fn eq_self_invocation_id_folds_true() {
    let g = Expr::InvocationId { axis: 0 };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Eq,
            left: Box::new(g.clone()),
            right: Box::new(g)
        }),
        Some(Expr::bool(true)),
        "Eq(InvocationId, InvocationId) is a sound reflexive fold (u32, never NaN)"
    );
}
#[test]
fn ne_self_invocation_id_folds_false() {
    let g = Expr::InvocationId { axis: 0 };
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ne,
            left: Box::new(g.clone()),
            right: Box::new(g)
        }),
        Some(Expr::bool(false)),
        "Ne(InvocationId, InvocationId) folds to false (u32, never NaN)"
    );
}
#[test]
fn lt_self_literal_folds_false() {
    // Integer-literal self-comparison stays foldable.
    let k = Expr::u32(7);
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Lt,
            left: Box::new(k.clone()),
            right: Box::new(k)
        }),
        Some(Expr::bool(false)),
        "Lt(7, 7) folds to false"
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
fn mod_self_var_does_not_fold() {
    // x % x must NOT fold to 0: const_fold is type/value-blind, and
    // signed `0 % 0` errors in the oracle (rem_i32). Folding to 0
    // fabricates a value where the i32 program is undefined.
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Mod,
            left: Box::new(x.clone()),
            right: Box::new(x)
        }),
        None,
        "Mod(Var, Var) must not fold: signed 0 % 0 is undefined in the oracle"
    );
}
#[test]
fn mod_literal_self_still_folds() {
    // The typed literal evaluator still folds concrete `k % k`, and
    // reproduces the oracle's unsigned `0 % 0 = 0` (rem_u32).
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Mod,
            left: Box::new(Expr::u32(6)),
            right: Box::new(Expr::u32(6))
        }),
        Some(Expr::u32(0)),
        "6 % 6 folds to 0 via the literal evaluator"
    );
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Mod,
            left: Box::new(Expr::u32(0)),
            right: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(0)),
        "unsigned 0 % 0 folds to 0, matching the rem_u32 oracle"
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
fn div_self_var_does_not_fold() {
    // x / x must NOT fold to 1: unsigned `0 / 0` is u32::MAX in the
    // oracle (div_u32) and the guarded SPIR-V emitter — not 1 — and
    // signed `0 / 0` errors (div_i32). const_fold cannot prove x != 0
    // or that x is unsigned, so folding to 1 is a miscompile for x=0.
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::div(x.clone(), x)),
        None,
        "Div(Var, Var) must not fold: 0 / 0 is u32::MAX (u32) or undefined (i32), never 1"
    );
}
#[test]
fn div_literal_self_still_folds() {
    // Concrete `k / k` still folds, and `0 / 0` reproduces the oracle's
    // unsigned u32::MAX (div_u32) rather than the bogus 1.
    assert_eq!(
        fold_expr(&Expr::div(Expr::u32(6), Expr::u32(6))),
        Some(Expr::u32(1)),
        "6 / 6 folds to 1 via the literal evaluator"
    );
    assert_eq!(
        fold_expr(&Expr::div(Expr::u32(0), Expr::u32(0))),
        Some(Expr::u32(u32::MAX)),
        "unsigned 0 / 0 folds to u32::MAX, matching the div_u32 oracle"
    );
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


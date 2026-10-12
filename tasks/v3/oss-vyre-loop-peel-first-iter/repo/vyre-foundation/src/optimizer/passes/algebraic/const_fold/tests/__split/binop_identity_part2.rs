use super::*;

#[test]
fn max_with_zero_is_identity() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Max,
            left: Box::new(x.clone()),
            right: Box::new(Expr::u32(0))
        }),
        Some(x.clone())
    );
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Max,
            left: Box::new(Expr::u32(0)),
            right: Box::new(x.clone())
        }),
        Some(x)
    );
}

#[test]
fn min_with_zero_is_zero() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Min,
            left: Box::new(x),
            right: Box::new(Expr::u32(0))
        }),
        Some(Expr::u32(0))
    );
}

#[test]
fn max_with_u32_max_is_u32_max() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Max,
            left: Box::new(x),
            right: Box::new(Expr::u32(u32::MAX))
        }),
        Some(Expr::u32(u32::MAX))
    );
}

#[test]
fn lt_zero_for_u32_is_false() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Lt,
            left: Box::new(x),
            right: Box::new(Expr::u32(0))
        }),
        Some(Expr::bool(false))
    );
}

#[test]
fn ge_zero_for_u32_is_true() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Ge,
            left: Box::new(x),
            right: Box::new(Expr::u32(0))
        }),
        Some(Expr::bool(true))
    );
}

#[test]
fn le_u32_max_is_true() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Le,
            left: Box::new(x),
            right: Box::new(Expr::u32(u32::MAX))
        }),
        Some(Expr::bool(true))
    );
}

#[test]
fn gt_u32_max_is_false() {
    let x = Expr::var("x");
    assert_eq!(
        fold_expr(&Expr::BinOp {
            op: crate::ir::BinOp::Gt,
            left: Box::new(x),
            right: Box::new(Expr::u32(u32::MAX))
        }),
        Some(Expr::bool(false))
    );
}

// ──── ROADMAP A33: distributive expansion for const-fold feed ────

/// `Mul(c, Add(a, k))` with both literals folds the right-side
/// product on the next pass: `c·a + c·k` → `c·a + (c*k)`. The rule
/// fires at the structural level here; the literal fold of
/// `Mul(c, Lit(k))` is the responsibility of the existing literal
/// evaluator.
#[test]
fn distributes_mul_lit_over_add_when_one_arm_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::u32(3)),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::u32(7)),
        }),
    });
    assert_eq!(
        folded,
        Some(Expr::add(
            Expr::mul(Expr::u32(3), Expr::var("x")),
            Expr::mul(Expr::u32(3), Expr::u32(7)),
        ))
    );
}

/// Symmetric: `Mul(Add(k, b), c)` distributes too.
#[test]
fn distributes_add_lit_times_mul_lit_when_one_arm_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::u32(5)),
            right: Box::new(Expr::var("y")),
        }),
        right: Box::new(Expr::u32(4)),
    });
    assert_eq!(
        folded,
        Some(Expr::add(
            Expr::mul(Expr::u32(5), Expr::u32(4)),
            Expr::mul(Expr::var("y"), Expr::u32(4)),
        ))
    );
}

/// i32 literals follow the same wrapping-integer arithmetic, so the
/// rewrite fires here too.
#[test]
fn distributes_mul_lit_i32_over_add_when_one_arm_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::i32(3)),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::i32(7)),
        }),
    });
    assert_eq!(
        folded,
        Some(Expr::add(
            Expr::mul(Expr::i32(3), Expr::var("x")),
            Expr::mul(Expr::i32(3), Expr::i32(7)),
        ))
    );
}

/// Negative: `Mul(c, Add(a, b))` where neither addend is a literal
/// must NOT distribute. Without a literal sibling there is no
/// guarantee the rewrite reduces instruction count post-fold, and
/// blind expansion would just bloat the IR.
#[test]
fn does_not_distribute_when_neither_addend_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::u32(3)),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::var("y")),
        }),
    });
    assert_eq!(folded, None);
}

/// Negative: `Mul(non-lit-c, Add(a, k))`  -  without a literal scalar
/// on the multiplied side, the rewrite would not fold either new
/// product, so the rule does not fire.
#[test]
fn does_not_distribute_when_scalar_is_not_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::var("c")),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::u32(7)),
        }),
    });
    assert_eq!(folded, None);
}

/// Negative: float multiplication is not associative under rounding,
/// so `f32 * (f32 + f32)` MUST NOT distribute even when literals are
/// present. The rounding path through one fused multiply differs
/// from two separate multiplies + an add.
#[test]
fn does_not_distribute_for_float_operands() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::f32(3.0)),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Add,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::f32(7.0)),
        }),
    });
    assert_eq!(folded, None);
}

/// Positive: `Mul` whose right side is `Sub` distributes (ROADMAP A33).
#[test]
fn distributes_mul_lit_over_sub_when_one_arm_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::u32(3)),
        right: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Sub,
            left: Box::new(Expr::var("x")),
            right: Box::new(Expr::u32(7)),
        }),
    });
    let expected = Expr::BinOp {
        op: crate::ir::BinOp::Sub,
        left: Box::new(Expr::mul(Expr::u32(3), Expr::var("x"))),
        right: Box::new(Expr::mul(Expr::u32(3), Expr::u32(7))),
    };
    assert_eq!(folded, Some(expected));
}

/// Symmetric: `Mul(Sub(k, b), c)` distributes too.
#[test]
fn distributes_sub_lit_times_mul_lit_when_one_arm_is_literal() {
    let folded = fold_expr(&Expr::BinOp {
        op: crate::ir::BinOp::Mul,
        left: Box::new(Expr::BinOp {
            op: crate::ir::BinOp::Sub,
            left: Box::new(Expr::u32(7)),
            right: Box::new(Expr::var("x")),
        }),
        right: Box::new(Expr::u32(3)),
    });
    let expected = Expr::BinOp {
        op: crate::ir::BinOp::Sub,
        left: Box::new(Expr::mul(Expr::u32(7), Expr::u32(3))),
        right: Box::new(Expr::mul(Expr::var("x"), Expr::u32(3))),
    };
    assert_eq!(folded, Some(expected));
}

// ─── ROADMAP A35: stronger range fold Mod(x, N) ───────────────────

fn test_mod_program(c: u32, n: u32) -> crate::optimizer::PassResult {
    use crate::ir::{BufferDecl, DataType, Node, Program};
    use crate::optimizer::passes::algebraic::const_fold::ConstFold;

    let entry = vec![
        Node::let_bind("x", Expr::u32(c)),
        Node::let_bind(
            "y",
            Expr::BinOp {
                op: crate::ir::BinOp::Mod,
                left: Box::new(Expr::var("x")),
                right: Box::new(Expr::u32(n)),
            },
        ),
        Node::store("out", Expr::u32(0), Expr::var("y")),
    ];
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        entry,
    );
    ConstFold::transform(program)
}

fn extract_let_y_value(nodes: &[crate::ir::Node]) -> Option<Expr> {
    for n in nodes {
        match n {
            crate::ir::Node::Let { name, value } if name.as_str() == "y" => {
                return Some(value.clone())
            }
            crate::ir::Node::Region { body, .. } => {
                if let Some(v) = extract_let_y_value(body) {
                    return Some(v);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn stronger_range_fold_mod_positive() {
    let result = test_mod_program(5, 10);
    assert!(result.changed);
    assert_eq!(
        extract_let_y_value(result.program.entry()),
        Some(Expr::var("x"))
    );
}

#[test]
fn stronger_range_fold_mod_negative_c_ge_n() {
    let result = test_mod_program(15, 10);
    // Not changed by lookbehind, but might be folded by normal literal const_fold.
    // If it is folded, it's 5. If not, it remains BinOp.
    // Either way, it doesn't fold to Var("x").
    let y = extract_let_y_value(result.program.entry());
    assert_ne!(y, Some(Expr::var("x")));
}

#[test]
fn stronger_range_fold_mod_negative_not_literal() {
    use crate::ir::{BufferDecl, DataType, Node, Program};
    use crate::optimizer::passes::algebraic::const_fold::ConstFold;

    let entry = vec![
        Node::let_bind("x", Expr::add(Expr::var("z"), Expr::u32(1))),
        Node::let_bind(
            "y",
            Expr::BinOp {
                op: crate::ir::BinOp::Mod,
                left: Box::new(Expr::var("x")),
                right: Box::new(Expr::u32(10)),
            },
        ),
    ];
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        entry,
    );
    let result = ConstFold::transform(program);
    assert!(!result.changed);
}

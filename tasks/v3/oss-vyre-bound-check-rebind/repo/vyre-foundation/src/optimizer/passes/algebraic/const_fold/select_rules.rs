// Select node simplifications.
//
// Covers: constant conditions, identical branches, condition inversion,
// and Select-to-Cast canonicalization.

use crate::ir::Expr;

/// Select simplifications  -  covers constant conditions, identical branches,
/// condition inversion, and Select-to-Cast canonicalization.
pub(super) fn simplify_select(cond: &Expr, true_val: &Expr, false_val: &Expr) -> Option<Expr> {
    use crate::ir::UnOp;
    match cond {
        Expr::LitBool(true) => Some(true_val.clone()),
        Expr::LitBool(false) => Some(false_val.clone()),
        Expr::LitU32(value) => {
            // Standard GPU u32→bool truthiness (target-text §6.7.1): 0 = false,
            // non-zero = true.
            if *value == 0 {
                Some(false_val.clone())
            } else {
                Some(true_val.clone())
            }
        }
        _ => {
            // Select(c, x, x) → x  (both branches identical)
            if true_val == false_val {
                return Some(true_val.clone());
            }
            // Select(!c, a, b) → Select(c, b, a)  (canonicalize condition)
            // Eliminates a LogicalNot instruction from the GPU shader.
            if let Expr::UnOp {
                op: UnOp::LogicalNot,
                operand: inner_cond,
            } = cond
            {
                return Some(Expr::select(
                    inner_cond.as_ref().clone(),
                    false_val.clone(),
                    true_val.clone(),
                ));
            }
            // Select(c, true, false) → Cast(bool→u32, c)  when branches
            // are the u32 encoding of a boolean result.
            if matches!(true_val, Expr::LitU32(1)) && matches!(false_val, Expr::LitU32(0)) {
                return Some(Expr::Cast {
                    target: crate::ir::DataType::U32,
                    value: Box::new(cond.clone()),
                });
            }
            // Select(c, false, true) → Cast(bool→u32, !c)
            if matches!(true_val, Expr::LitU32(0)) && matches!(false_val, Expr::LitU32(1)) {
                return Some(Expr::Cast {
                    target: crate::ir::DataType::U32,
                    value: Box::new(Expr::UnOp {
                        op: UnOp::LogicalNot,
                        operand: Box::new(cond.clone()),
                    }),
                });
            }
            // ─── Select-of-Select fusion ─────────────────────
            // Select(c, Select(c, a, b), d) → Select(c, a, d)
            // When the outer and inner Select share the same condition,
            // the inner true-branch is always taken, so the inner
            // false-branch is dead.
            if let Expr::Select {
                cond: inner_cond,
                true_val: inner_true,
                ..
            } = true_val
            {
                if inner_cond.as_ref() == cond {
                    return Some(Expr::select(
                        cond.clone(),
                        inner_true.as_ref().clone(),
                        false_val.clone(),
                    ));
                }
            }
            // Select(c, d, Select(c, a, b)) → Select(c, d, b)
            if let Expr::Select {
                cond: inner_cond,
                false_val: inner_false,
                ..
            } = false_val
            {
                if inner_cond.as_ref() == cond {
                    return Some(Expr::select(
                        cond.clone(),
                        true_val.clone(),
                        inner_false.as_ref().clone(),
                    ));
                }
            }
            None
        }
    }
}

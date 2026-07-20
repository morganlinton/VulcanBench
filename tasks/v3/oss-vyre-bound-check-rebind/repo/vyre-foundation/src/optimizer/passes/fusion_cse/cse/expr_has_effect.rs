//! Effect classification for common-subexpression elimination.

use crate::ir::Expr;

/// Return true when evaluating `expr` can read or mutate external state.
#[must_use]
#[inline]
pub fn expr_has_effect(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic { .. } | Expr::Call { .. } => true,
        Expr::Load { index, .. }
        | Expr::UnOp { operand: index, .. }
        | Expr::Cast { value: index, .. } => expr_has_effect(index),
        Expr::BinOp { left, right, .. } => expr_has_effect(left) || expr_has_effect(right),
        Expr::Fma { a, b, c } => expr_has_effect(a) || expr_has_effect(b) || expr_has_effect(c),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => expr_has_effect(cond) || expr_has_effect(true_val) || expr_has_effect(false_val),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. } => false,
        Expr::Opaque(extension) => !extension.cse_safe(),
    }
}

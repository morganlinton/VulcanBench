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
        // A subgroup op's EFFECT is its operand's effect. `CseCtx::expr` does
        // not descend into a subgroup op's operand (the subgroup op interns to
        // a unique key, so it is never itself deduplicated), which means the
        // enclosing node relies on this classification to decide whether to
        // invalidate observed state. If an operand performs a write
        // (`Atomic`/`Call`, or a `Load` over an effectful index) and this
        // returned `false`, a value cached before the op would be wrongly
        // reused after it — e.g. the stream-compaction idiom
        // `SubgroupReduce(Add, Atomic(FetchAdd, ctr, 1))` mutates `ctr` but a
        // prior `Load(ctr)` would survive as a stale CSE alias. Recurse,
        // mirroring the `Load`/`UnOp` handling above.
        Expr::SubgroupBallot { cond } => expr_has_effect(cond),
        Expr::SubgroupShuffle { value, lane } => {
            expr_has_effect(value) || expr_has_effect(lane)
        }
        Expr::SubgroupReduce { value, .. } => expr_has_effect(value),
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
        | Expr::SubgroupSize => false,
        Expr::Opaque(extension) => !extension.cse_safe(),
    }
}

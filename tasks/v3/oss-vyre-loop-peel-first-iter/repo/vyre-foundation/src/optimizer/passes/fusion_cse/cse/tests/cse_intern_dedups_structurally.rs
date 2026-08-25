//! Integration test crate for the containing Vyre package.

use crate::ir::Expr;
use crate::optimizer::passes::fusion_cse::cse::CseCtx;

/// O15 (revised): CSE deduplicates structural keys, not pointer addresses.
///
/// The original O15 implementation cached by `*const Expr -> ExprId`,
/// which was unsound: `Box<Expr>` sub-trees freed by `Cow::Owned`
/// rewrites in `CseCtx::expr` get reallocated at the same address by
/// later sub-trees, so the cache returned stale ExprIds and CSE
/// merged semantically distinct expressions (caught by
/// `full_optimize_is_idempotent_on_canonical_wire` shrinking
/// `BitAnd(46, Mul(BitAnd(888, X), 0))` into the unrelated
/// `BitOr(invocation, 1)`).
///
/// The corrected contract: structurally identical sub-expressions
/// return the SAME `ExprId` from `intern_expr`. The pointer is no
/// longer cached, but the `deduplication` `FxHashMap<ExprKey, ExprId>`
/// still gives O(1) intern dedup by structural key.
#[test]
#[inline]
fn cse_intern_dedups_structurally_equivalent_subtrees() {
    let mut ctx = CseCtx::default();

    let a = Expr::add(Expr::u32(1), Expr::u32(2));
    let b = Expr::add(Expr::u32(1), Expr::u32(2));

    let id_a = ctx.intern_expr(&a);
    let id_b = ctx.intern_expr(&b);

    assert_eq!(
        id_a, id_b,
        "Fix: structurally identical Add(LitU32(1), LitU32(2)) trees must share an ExprId."
    );

    // Different operands → different IDs.
    let c = Expr::add(Expr::u32(1), Expr::u32(3));
    let id_c = ctx.intern_expr(&c);
    assert_ne!(id_a, id_c, "Fix: distinct Adds must get distinct ExprIds.");
}

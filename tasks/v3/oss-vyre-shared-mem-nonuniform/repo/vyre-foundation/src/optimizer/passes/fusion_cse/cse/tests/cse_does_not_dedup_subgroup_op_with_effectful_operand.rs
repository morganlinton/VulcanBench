//! Integration test crate for the containing Vyre package.
//!
//! A subgroup op whose operand performs a memory WRITE must invalidate the CSE
//! value cache, exactly as a bare write would. `expr_has_effect` previously
//! classified every `SubgroupBallot`/`Shuffle`/`Reduce` as pure WITHOUT
//! inspecting its operand, and `CseCtx::expr` does not descend into a subgroup
//! op's operand either — so `let _ = SubgroupReduce(Add, Atomic(FetchAdd, buf,
//! 0, 1))` left the cache untouched. A `Load(buf, 0)` cached before it was then
//! wrongly reused afterward, reading the stale pre-atomic value (a miscompile).
//! The fix makes `expr_has_effect` recurse into subgroup operands, so the
//! enclosing `let` invalidates observed state. (The subgroup op itself is never
//! deduplicated — `intern_expr` gives each a unique key — so this is purely
//! about the missed invalidation.)

use crate::ir::{BufferDecl, DataType, Expr, Node, Program, SubgroupReduceOp};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

#[test]
#[inline]
fn cse_subgroup_op_with_atomic_write_invalidates_load_cache() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("buf", 0, DataType::U32),
            BufferDecl::read_write("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            // Cache Load(buf, 0) under `pre`.
            Node::let_bind("pre", Expr::load("buf", Expr::u32(0))),
            // Atomic fetch-add to buf[0], wrapped in a subgroup reduction.
            // This MUTATES buf[0], so any cached Load(buf, 0) is now stale.
            Node::let_bind(
                "r",
                Expr::subgroup_reduce(
                    SubgroupReduceOp::Add,
                    Expr::atomic_add("buf", Expr::u32(0), Expr::u32(1)),
                ),
            ),
            // Re-read buf[0]: must be a fresh Load, NOT aliased to `pre`.
            Node::let_bind("post", Expr::load("buf", Expr::u32(0))),
            Node::store("out", Expr::u32(0), Expr::var("pre")),
            Node::store("out", Expr::u32(1), Expr::var("post")),
        ],
    );

    let optimized = cse(program);
    let body = crate::test_util::region_body(&optimized);

    // entry[2] is `let post = ...`. Pre-fix it was rewritten to `let post = pre`
    // because the atomic write (hidden inside the subgroup op) never invalidated
    // the cached Load(buf, 0). It must remain a Load reading the current value.
    let Node::Let { name, value } = &body[2] else {
        panic!("expected entry[2] to be `let post = ...`, got {:?}", body[2]);
    };
    assert_eq!(name.as_str(), "post", "entry[2] must be the post-atomic load");
    assert!(
        matches!(value, Expr::Load { buffer, .. } if buffer.as_str() == "buf"),
        "the post-atomic re-read must stay a fresh Load(buf, 0), not alias the \
         stale pre-atomic value; got {value:?}"
    );
}

//! Integration test crate for the containing Vyre package.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

/// Regression: CSE must NOT replace a literal with a variable reference.
///
/// Before the fix, `let state = 0u; ... for(step = 0u; ...)` would CSE the
/// loop's `from: Expr::u32(0)` into `Expr::var("state")`. When `state` is
/// later reassigned inside the loop, the iterator init reads a stale value.
#[test]
#[inline]
fn cse_does_not_alias_literal_through_mutable_variable() -> Result<(), String> {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read("buf", 0, DataType::U32)],
        [256, 1, 1],
        vec![
            Node::let_bind("state", Expr::u32(0)),
            Node::loop_for(
                "step",
                Expr::u32(0),
                Expr::load("buf", Expr::u32(0)),
                vec![Node::assign("state", Expr::load("buf", Expr::var("step")))],
            ),
        ],
    ));
    let optimized = cse(program);
    // The loop's `from` expression must remain a literal  -  not `Var("state")`.
    let body = crate::test_util::region_body(&optimized);
    match &body[1] {
        Node::Loop { from, .. } => {
            assert!(
                matches!(from, Expr::LitU32(0)),
                "CSE replaced loop `from` literal with {from:?}  -  this is unsound because the \
                 original let-bind target may be reassigned inside the loop"
            );
        }
        other => {
            return Err(format!(
                "Fix: expected optimized entry[1] to remain a Loop node, got {other:?}"
            ));
        }
    }
    Ok(())
}

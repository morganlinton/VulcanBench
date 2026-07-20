//! Integration test crate for the containing Vyre package.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

/// CSE should still eliminate non-literal duplicated expressions.
#[test]
#[inline]
fn cse_still_eliminates_non_literal_common_subexpressions() -> Result<(), String> {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read_write("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
            Node::let_bind("y", Expr::load("a", Expr::u32(0))),
            Node::store("out", Expr::u32(0), Expr::var("x")),
        ],
    ));
    let optimized = cse(program);
    // The second let_bind should reuse `x` instead of re-loading.
    let body = crate::test_util::region_body(&optimized);
    match &body[1] {
        Node::Let { value, .. } => {
            assert!(
                matches!(value, Expr::Var(name) if name == "x"),
                "CSE should have deduplicated Load to Var(\"x\"), got {value:?}"
            );
        }
        other => {
            return Err(format!(
                "Fix: expected optimized entry[1] to remain a Let node, got {other:?}"
            ));
        }
    }
    Ok(())
}

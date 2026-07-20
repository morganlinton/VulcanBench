//! Integration test crate for the containing Vyre package.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

#[test]
#[inline]
fn cse_scoped_side_effect_invalidates_without_leaking() -> Result<(), String> {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::read_write("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("outer", Expr::load("a", Expr::u32(0))),
            Node::if_then_else(
                Expr::bool(true),
                vec![
                    Node::let_bind("before_store", Expr::load("a", Expr::u32(0))),
                    Node::store("out", Expr::u32(0), Expr::var("before_store")),
                    Node::let_bind("after_store", Expr::load("a", Expr::u32(0))),
                ],
                vec![],
            ),
        ],
    ));

    let optimized = cse(program);
    let body = crate::test_util::region_body(&optimized);
    let Node::If { then, .. } = &body[1] else {
        return Err(format!(
            "Fix: expected optimized entry[1] to remain If, got {:?}",
            body[1]
        ));
    };

    match &then[0] {
        Node::Let { value, .. } => assert!(
            matches!(value, Expr::Var(name) if name == "outer"),
            "Fix: branch load before the store should CSE to the visible outer load, got {value:?}"
        ),
        other => return Err(format!("Fix: expected then[0] Let, got {other:?}")),
    }

    match &then[2] {
        Node::Let { value, .. } => assert!(
            matches!(value, Expr::Load { .. }),
            "Fix: branch load after the store must not reuse the pre-store value, got {value:?}"
        ),
        other => return Err(format!("Fix: expected then[2] Let, got {other:?}")),
    }

    Ok(())
}

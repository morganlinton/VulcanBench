//! Var-to-var CSE must preserve temporal snapshot boundaries.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

#[test]
fn cse_does_not_alias_var_snapshot_through_mutable_source() {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("s0", Expr::u32(1)),
            Node::let_bind("s1", Expr::u32(2)),
            Node::let_bind("tmp", Expr::var("s0")),
            Node::assign("s0", Expr::var("s1")),
            Node::assign("s1", Expr::var("tmp")),
            Node::store("out", Expr::u32(0), Expr::var("s1")),
        ],
    ));

    let optimized = cse(program);
    let body = crate::test_util::region_body(&optimized);

    assert!(
        body.iter().any(|node| matches!(
            node,
            Node::Let { name, value: Expr::Var(source) }
                if name.as_str() == "tmp" && source.as_str() == "s0"
        )),
        "Fix: CSE must not rewrite `let tmp = s0` into an alias when `s0` is reassigned before `tmp` is consumed"
    );
}

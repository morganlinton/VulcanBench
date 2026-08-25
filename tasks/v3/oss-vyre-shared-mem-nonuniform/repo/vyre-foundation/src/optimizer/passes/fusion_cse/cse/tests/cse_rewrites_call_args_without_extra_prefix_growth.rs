//! Integration test crate for the containing Vyre package.

use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
use crate::optimizer::passes::fusion_cse::cse::engine::cse;

#[test]
#[inline]
fn cse_rewrites_call_args_without_extra_prefix_growth() -> Result<(), String> {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read("a", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("a", Expr::u32(0))),
            Node::let_bind(
                "y",
                Expr::Call {
                    op_id: "vyre.test.call".into(),
                    args: vec![
                        Expr::u32(7),
                        Expr::load("a", Expr::u32(0)),
                        Expr::add(Expr::load("a", Expr::u32(0)), Expr::u32(1)),
                    ],
                },
            ),
        ],
    ));

    let optimized = cse(program);
    let body = crate::test_util::region_body(&optimized);
    let Node::Let { value, .. } = &body[1] else {
        return Err(format!(
            "Fix: expected optimized entry[1] to remain Let, got {:?}",
            body[1]
        ));
    };
    let Expr::Call { args, .. } = value else {
        return Err(format!(
            "Fix: expected optimized value to remain Call, got {value:?}"
        ));
    };

    assert!(
        matches!(&args[1], Expr::Var(name) if name == "x"),
        "Fix: CSE should rewrite repeated call arg load to x, got {:?}",
        args[1]
    );
    assert_eq!(args.len(), 3);
    Ok(())
}

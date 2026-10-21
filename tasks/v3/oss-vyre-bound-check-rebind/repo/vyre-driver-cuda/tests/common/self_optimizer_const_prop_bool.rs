use crate::{body_of, run_pipeline};
use vyre::ir::model::types::UnOp;
use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::BinOp {
        op,
        left: Box::new(a),
        right: Box::new(b),
    }
}

fn bool_false_comparison_program(cond: Expr) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "b",
                Expr::eq(Expr::load("input", Expr::u32(0)), Expr::u32(7)),
            ),
            Node::if_then_else(
                cond,
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(2))],
            ),
        ],
    )
}

#[test]
fn cuda_const_prop_simplifies_bool_false_comparisons_to_logical_not() {
    for (label, cond) in [
        (
            "b == false",
            binop(BinOp::Eq, Expr::var("b"), Expr::bool(false)),
        ),
        (
            "false == b",
            binop(BinOp::Eq, Expr::bool(false), Expr::var("b")),
        ),
        (
            "b != true",
            binop(BinOp::Ne, Expr::var("b"), Expr::bool(true)),
        ),
        (
            "true != b",
            binop(BinOp::Ne, Expr::bool(true), Expr::var("b")),
        ),
    ] {
        let out = run_pipeline(bool_false_comparison_program(cond));
        let body = body_of(&out);
        let if_node = body.iter().find(|n| matches!(n, Node::If { .. }));
        if let Some(Node::If { cond, .. }) = if_node {
            assert!(
                matches!(
                    cond,
                    Expr::UnOp {
                        op: UnOp::LogicalNot,
                        ..
                    }
                ),
                "{label} must simplify to LogicalNot(b); got cond={cond:?}"
            );
        } else {
            panic!("{label} must preserve a runtime If with simplified condition; body={body:?}");
        }
    }
}

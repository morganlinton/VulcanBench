//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn optimizer_strength_reduce_multiplies_by_two() {
    let program = crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::mul(Expr::var("x"), Expr::u32(2)),
        )],
    ));

    let optimized = PassScheduler::with_passes(vec![
        ProgramPassKind::new(ConstFold),
        ProgramPassKind::new(StrengthReduce),
    ])
    .run(program)
    .expect("Fix: strength reduce should converge");

    let body = crate::test_util::region_body(&optimized);
    assert!(matches!(
        &body[0],
        Node::Store {
            value: Expr::BinOp {
                op: BinOp::Shl,
                right,
                ..
            },
            ..
        } if matches!(right.as_ref(), Expr::LitU32(1))
    ));
}

#[test]
fn optimizer_strength_reduce_decomposes_mul_by_three() {
    // x * 3 should now decompose to a bounded shift/sub chain.
    let program = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind(
            "x",
            Expr::mul(Expr::var("input"), Expr::u32(3)),
        )],
    );

    let optimized = PassScheduler::with_passes(vec![
        ProgramPassKind::new(ConstFold),
        ProgramPassKind::new(StrengthReduce),
    ])
    .run(program)
    .expect("Fix: strength reduce should converge");

    let body = crate::test_util::region_body(&optimized);
    assert!(
        matches!(
            &body[0],
            Node::Let {
                value: Expr::BinOp {
                    op: BinOp::Add | BinOp::Sub,
                    ..
                },
                ..
            }
        ),
        "x * 3 must decompose to a shift/add/sub chain: {body:?}"
    );
}

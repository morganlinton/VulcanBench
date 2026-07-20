use super::*;

#[test]
fn cuda_bitxor_chain_cancels_right_via_cse() {
    // Build `let y = Load(input, 0); store buf 0 ((x ^ y) ^ y)`
    //  -  both `y` operands are CSE-equivalent so the outer BitXor
    // cancels the inner pair and leaves `x`.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::let_bind("y", Expr::load("input", Expr::u32(0))),
            Node::store(
                "buf",
                Expr::u32(0),
                binop(
                    BinOp::BitXor,
                    binop(BinOp::BitXor, Expr::var("x"), Expr::var("y")),
                    Expr::var("y"),
                ),
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        // After CSE proves x and y both alias Load(input,0) and the
        // outer BitXor folds, what remains is `Var(x)` (or potentially
        // const-prop'd to a single Load reference). Both forms pass.
        assert!(
            !matches!(
                value,
                Expr::BinOp {
                    op: BinOp::BitXor,
                    ..
                }
            ),
            "BitXor chain must collapse; got {value:?}"
        );
    }
}

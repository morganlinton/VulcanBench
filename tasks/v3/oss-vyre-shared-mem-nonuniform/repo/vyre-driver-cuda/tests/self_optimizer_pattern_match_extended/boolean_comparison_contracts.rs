use super::*;

#[test]
fn cuda_eq_self_collapses_to_true_via_cse() {
    // store buf 0 (var("x") == var("x"))  →  store buf 0 LitBool(true)
    let p = program_with_x_load_then(Expr::eq(Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitBool(true)),
            "expected LitBool(true) after `x == x` collapse via CSE; got {value:?}"
        );
    }
}

#[test]
fn cuda_bool_and_self_collapses_via_cse() {
    // (b && b) → b. Both operands are Var(b), CSE proves equality.
    // Use the cond inside an If to drive a Store decision.
    let p = Program::wrapped(
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
                binop(BinOp::And, Expr::var("b"), Expr::var("b")),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(2))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // The (b && b) → b rewrite produces an If whose cond is Var(b)
    // (post-rewrite)  -  the cond no longer has BinOp::And at the top.
    let if_node = body.iter().find(|n| matches!(n, Node::If { .. }));
    if let Some(Node::If { cond, .. }) = if_node {
        assert!(
            !matches!(cond, Expr::BinOp { op: BinOp::And, .. }),
            "(b && b) must collapse; got cond={cond:?}"
        );
    }
}

#[test]
fn cuda_bool_and_with_false_collapses_to_false() {
    // (Var(b) && false) → false. The store's value is non-bool so
    // we put the test under an If cond.
    let p = Program::wrapped(
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
                binop(BinOp::And, Expr::var("b"), Expr::bool(false)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "(b && false) must fold to false and drop the If; body={body:?}"
    );
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(99)),
            "(b && false) → false should pick else; got {value:?}"
        );
    }
}

#[test]

fn cuda_bool_or_with_true_collapses_to_true() {
    // (Var(b) || true) → true. Pick the then arm.
    let p = Program::wrapped(
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
                binop(BinOp::Or, Expr::var("b"), Expr::bool(true)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "(b || true) must fold to true and drop the If; body={body:?}"
    );
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(1)),
            "(b || true) → true should pick then; got {value:?}"
        );
    }
}

#[test]
fn cuda_gt_self_collapses_to_false_via_cse() {
    // (var("x") > var("x")) → LitBool(false). Adversarial: catches
    // the previous miswiring where `is_cmp_gt` was bound to the
    // wrong op tag, which would've collapsed `Gt(x,x)` to `true`.
    let p = program_with_x_load_then(binop(BinOp::Gt, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitBool(false)),
            "Gt(x,x) must fold to false; got {value:?}"
        );
    }
}

#[test]
fn cuda_le_self_collapses_to_true_via_cse() {
    let p = program_with_x_load_then(binop(BinOp::Le, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitBool(true)),
            "Le(x,x) must fold to true; got {value:?}"
        );
    }
}

#[test]
fn cuda_ge_self_collapses_to_true_via_cse() {
    let p = program_with_x_load_then(binop(BinOp::Ge, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitBool(true)),
            "Ge(x,x) must fold to true; got {value:?}"
        );
    }
}

#[test]
fn cuda_lt_self_collapses_to_false_via_cse() {
    // store buf 0 (var("x") < var("x"))  →  store buf 0 LitBool(false)
    let p = program_with_x_load_then(Expr::lt(Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitBool(false)),
            "expected LitBool(false) after `x < x` collapse via CSE; got {value:?}"
        );
    }
}


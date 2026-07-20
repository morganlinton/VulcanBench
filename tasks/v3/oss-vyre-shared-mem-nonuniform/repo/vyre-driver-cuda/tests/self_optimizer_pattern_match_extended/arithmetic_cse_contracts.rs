use super::*;

#[test]
fn cuda_sub_add_cancel_right_via_cse() {
    // store buf 0 ((Var(x) + Var(y)) - Var(y))  →  store buf 0 Var(x)
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("inx", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("iny", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("inx", Expr::u32(0))),
            Node::let_bind("y", Expr::load("iny", Expr::u32(0))),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::sub(Expr::add(Expr::var("x"), Expr::var("y")), Expr::var("y")),
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
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after `(x+y)-y` collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_add_sub_cancel_via_cse() {
    // store buf 0 ((Var(x) - Var(y)) + Var(y))  →  store buf 0 Var(x)
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("inx", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("iny", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("inx", Expr::u32(0))),
            Node::let_bind("y", Expr::load("iny", Expr::u32(0))),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::add(Expr::sub(Expr::var("x"), Expr::var("y")), Expr::var("y")),
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
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after `(x-y)+y` collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_div_by_one_collapses_to_left() {
    // store buf 0 (var("x") / 1) → store buf 0 var("x")
    let p = program_with_x_load_then(Expr::div(Expr::var("x"), Expr::u32(1)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Div-by-1 collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_mod_by_one_collapses_to_zero() {
    // store buf 0 (var("x") % 1) → store buf 0 0
    let p = program_with_x_load_then(Expr::rem(Expr::var("x"), Expr::u32(1)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after Mod-by-1 collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_double_abs_does_not_collapse_to_inner() {
    // Abs is idempotent (Abs(Abs(x)) == Abs(x)), NOT involutive
    // (Abs(Abs(x)) ≠ x in general). Adversarial test: catches a
    // previous bug where the UnOp double-application matcher fired
    // for any same-op pair, incorrectly collapsing Abs(Abs(x)) → x.
    use vyre::ir::model::types::UnOp;
    let p = program_with_x_load_then(Expr::UnOp {
        op: UnOp::Abs,
        operand: Box::new(Expr::UnOp {
            op: UnOp::Abs,
            operand: Box::new(Expr::var("x")),
        }),
    });
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        // Either the outer Abs is preserved (correct shape) OR it
        // collapsed to the inner Abs (also correct since Abs is
        // idempotent). Either way, the result must NOT be raw Var(x).
        assert!(
            !matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "Abs(Abs(x)) must not collapse to Var(x); got {value:?}"
        );
    }
}



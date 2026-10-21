use super::*;

#[test]
fn cuda_sub_zero_collapses_to_left() {
    // store buf 0 (var("x") - 0)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::sub(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Sub-zero collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_bitand_zero_collapses_to_zero() {
    // store buf 0 (var("x") & 0)  →  store buf 0 0
    let p = program_with_x_load_then(Expr::bitand(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after BitAnd-zero collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_bitor_zero_collapses_to_left() {
    // store buf 0 (var("x") | 0)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::bitor(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after BitOr-zero collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_sub_add_cancel_left_via_cse() {
    // store buf 0 ((Var(x) + Var(y)) - Var(x))  →  store buf 0 Var(y)
    // Both x and y are bound to non-literal Loads so they survive
    // const-prop and remain Var refs.
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
                Expr::sub(Expr::add(Expr::var("x"), Expr::var("y")), Expr::var("x")),
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
            matches!(value, Expr::Var(n) if n.as_str() == "y"),
            "expected Var(y) after `(x+y)-x` collapse; got {value:?}"
        );
    }
}



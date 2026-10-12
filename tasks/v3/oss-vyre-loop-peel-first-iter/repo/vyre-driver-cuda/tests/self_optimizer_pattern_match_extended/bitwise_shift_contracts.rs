use super::*;

#[test]
fn cuda_double_bitnot_collapses() {
    // store buf 0 (~~ var("x"))  →  store buf 0 var("x")
    use vyre::ir::UnOp;
    let p = program_with_x_load_then(Expr::UnOp {
        op: UnOp::BitNot,
        operand: Box::new(Expr::UnOp {
            op: UnOp::BitNot,
            operand: Box::new(Expr::var("x")),
        }),
    });
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after `~~x` collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_bitand_max_collapses_to_left() {
    // store buf 0 (var("x") & u32::MAX)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::bitand(Expr::var("x"), Expr::u32(u32::MAX)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after BitAnd-MAX collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_shl_zero_collapses_to_left() {
    // store buf 0 (var("x") << 0)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::shl(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Shl-by-zero collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_shr_zero_collapses_to_left() {
    // store buf 0 (var("x") >> 0)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::shr(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Shr-by-zero collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_zero_shl_collapses_to_zero() {
    // store buf 0 (0u32 << var("x"))  →  store buf 0 0
    let p = program_with_x_load_then(Expr::shl(Expr::u32(0), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after `0 << x` collapse; got {value:?}"
        );
    }
}



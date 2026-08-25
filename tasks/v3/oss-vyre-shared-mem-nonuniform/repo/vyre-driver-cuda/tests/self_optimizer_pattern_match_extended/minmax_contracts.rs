use super::*;

#[test]
fn cuda_min_with_zero_collapses_to_zero() {
    // Min(x, 0u) → 0u (u32 minimum is 0).
    let p = program_with_x_load_then(binop(BinOp::Min, Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "Min(x, 0) must fold to 0; got {value:?}"
        );
    }
}

#[test]
fn cuda_max_with_max_collapses_to_max() {
    // Max(x, MAX) → MAX (u32 maximum saturates).
    let p = program_with_x_load_then(binop(BinOp::Max, Expr::var("x"), Expr::u32(u32::MAX)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        match value {
            Expr::LitU32(v) if *v == u32::MAX => {}
            other => panic!("Max(x, MAX) must fold to MAX; got {other:?}"),
        }
    }
}

#[test]
fn cuda_min_with_max_collapses_to_left() {
    // Min(x, MAX) → x (clamp to MAX is a no-op).
    let p = program_with_x_load_then(binop(BinOp::Min, Expr::var("x"), Expr::u32(u32::MAX)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "Min(x, MAX) must fold to x; got {value:?}"
        );
    }
}

#[test]
fn cuda_max_with_zero_collapses_to_left() {
    // Max(x, 0u) → x (clamp from below by 0 is a no-op for u32).
    let p = program_with_x_load_then(binop(BinOp::Max, Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "Max(x, 0) must fold to x; got {value:?}"
        );
    }
}

#[test]
fn cuda_min_self_collapses_via_cse() {
    // store buf 0 (min(x, x)) → store buf 0 var("x")
    let p = program_with_x_load_then(binop(BinOp::Min, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Min-self collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_max_self_collapses_via_cse() {
    let p = program_with_x_load_then(binop(BinOp::Max, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after Max-self collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_absdiff_self_collapses_to_zero() {
    let p = program_with_x_load_then(binop(BinOp::AbsDiff, Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after AbsDiff-self collapse; got {value:?}"
        );
    }
}

#[test]
fn cuda_bitxor_zero_collapses_to_left() {
    // store buf 0 (var("x") ^ 0)  →  store buf 0 var("x")
    let p = program_with_x_load_then(Expr::bitxor(Expr::var("x"), Expr::u32(0)));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after BitXor-zero collapse; got {value:?}"
        );
    }
}


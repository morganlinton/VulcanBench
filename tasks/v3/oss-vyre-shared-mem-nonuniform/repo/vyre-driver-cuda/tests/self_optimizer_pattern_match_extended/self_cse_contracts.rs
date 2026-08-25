use super::*;

#[test]
fn cuda_xor_self_collapses_via_cse() {
    // store buf 0 (var("x") ^ var("x")) → store buf 0 0
    // Requires CSE-aware pattern_match: canonical[arg1] == canonical[arg2].
    let p = program_with_x_load_then(Expr::bitxor(Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after BitXor-self collapse via CSE; got {value:?}"
        );
    }
}

#[test]
fn cuda_sub_self_collapses_via_cse() {
    // store buf 0 (var("x") - var("x")) → store buf 0 0
    let p = program_with_x_load_then(Expr::sub(Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after Sub-self collapse via CSE; got {value:?}"
        );
    }
}

#[test]
fn cuda_bitand_self_collapses_via_cse() {
    // store buf 0 (var("x") & var("x")) → store buf 0 var("x")
    let p = program_with_x_load_then(Expr::bitand(Expr::var("x"), Expr::var("x")));
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "expected Var(x) after BitAnd-self collapse via CSE; got {value:?}"
        );
    }
}


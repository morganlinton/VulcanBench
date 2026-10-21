//! Adversarial tests for validator depth-limit enforcement.
//!
//! The validator must reject programs that exceed the configured
//! maximum call depth, expression depth, nesting depth, or node count.
//! These limits prevent pathological inputs from causing stack
//! overflow or unbounded computation during optimization.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::validate;

fn output_buf() -> BufferDecl {
    BufferDecl::output("out", 0, DataType::U32).with_count(1)
}

#[test]
fn deeply_nested_if_exceeds_nesting_limit() {
    // Build a program with 100 nested If nodes.
    let mut body = vec![Node::Return];
    for _ in 0..100 {
        body = vec![Node::if_then(Expr::bool(true), body)];
    }
    let program = Program::wrapped(vec![output_buf()], [1, 1, 1], body);

    let errors = validate(&program);
    assert!(
        !errors.is_empty(),
        "deeply nested If (100 levels) must exceed nesting limit, got no errors"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("depth") || e.message().contains("limit")),
        "depth-limit error must mention 'depth' or 'limit', got: {:?}",
        errors
    );
}

#[test]
fn deeply_nested_loop_exceeds_nesting_limit() {
    // Build a program with 100 nested Loop nodes.
    let mut body = vec![Node::Return];
    for _ in 0..100 {
        body = vec![Node::loop_for("i", Expr::u32(0), Expr::u32(1), body)];
    }
    let program = Program::wrapped(vec![output_buf()], [1, 1, 1], body);

    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("depth") || e.message().contains("limit")),
        "deeply nested Loop (100 levels) must exceed nesting limit, got: {:?}",
        errors
    );
}

#[test]
fn huge_node_count_exceeds_limit() {
    // The statement-node ceiling is deliberately large — 100_000, the value of
    // vyre-foundation's validate::depth::DEFAULT_MAX_NODE_COUNT, which must admit
    // a fully fused megakernel bundle AND agree with the substrate's GPU-native
    // encoded validator. (The test asserts against the vyre-core public boundary,
    // which does not re-export the const, so the ceiling is pinned as a literal;
    // the substrate parity evidence guards the two validators against drift.)
    // Build just past it so the test exercises the real V019 path rather than an
    // arbitrary smaller threshold the validator would correctly accept.
    const MAX_NODE_COUNT: usize = 100_000;
    let node_count = MAX_NODE_COUNT + 1;
    let mut body = Vec::with_capacity(node_count + 1);
    for i in 0..node_count {
        body.push(Node::let_bind(format!("v{i}"), Expr::u32(i as u32)));
    }
    body.push(Node::Return);
    let program = Program::wrapped(vec![output_buf()], [1, 1, 1], body);

    let errors = validate(&program);
    assert!(
        !errors.is_empty(),
        "program with {node_count} nodes must exceed node-count limit {MAX_NODE_COUNT}, got no errors"
    );
    assert!(
        errors.iter().any(|e| {
            let m = e.message();
            m.contains("V019") && m.contains("node count") && m.contains("limit")
        }),
        "node-count error must be V019 naming 'node count' and 'limit', got: {errors:?}"
    );
}

#[test]
fn deep_expr_tree_exceeds_expr_depth_limit() {
    // Build an expression with 200 nested Add operations.
    let mut expr = Expr::u32(0);
    for _ in 0..200 {
        expr = Expr::add(expr, Expr::u32(1));
    }
    let program = Program::wrapped(
        vec![output_buf()],
        [1, 1, 1],
        vec![Node::let_bind("deep", expr), Node::Return],
    );

    let errors = validate(&program);
    assert!(
        !errors.is_empty(),
        "expression with depth 200 must exceed expr-depth limit, got no errors"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("expr") || e.message().contains("depth")),
        "expr-depth error must mention 'expr' or 'depth', got: {:?}",
        errors
    );
}

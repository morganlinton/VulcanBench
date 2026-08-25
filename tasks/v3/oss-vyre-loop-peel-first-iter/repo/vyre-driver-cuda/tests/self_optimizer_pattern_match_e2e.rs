//! E2E test: hardcoded algebraic-identity pattern-match running as a
//! vyre Program on real CUDA hardware. Same kernel as the wgpu test.

#![cfg(test)]

mod common;

use common::{live_backend, CudaOptimizerDispatcher};
use vyre::ir::{BinOp, Expr, Node, Program};
use vyre_self_substrate::optimizer::pattern_match_via_encoded::gpu_algebraic_identities;

fn wrapped(entry: Vec<Node>) -> Program {
    Program::wrapped(Vec::new(), [1, 1, 1], entry)
}

fn first_let_value(p: &Program) -> Expr {
    match p.entry() {
        [Node::Region { body, .. }] => match body.as_slice() {
            [Node::Let { value, .. }] => value.clone(),
            _ => panic!("expected single Let in body"),
        },
        _ => panic!("expected wrapped Program"),
    }
}

#[test]
fn cuda_add_zero_left_collapses() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(0), Expr::var("a")),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::Var(ref n) if n.as_str() == "a"));
}

#[test]
fn cuda_mul_zero_absorbs() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::u32(0), Expr::var("a")),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::LitU32(0)));
}

#[test]
fn cuda_mul_one_collapses() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::var("a"), Expr::u32(1)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::Var(ref n) if n.as_str() == "a"));
}

#[test]
fn cuda_unrelated_binop_passes_through() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::sub(Expr::var("a"), Expr::u32(1)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    match got {
        Expr::BinOp { op, .. } => assert!(matches!(op, BinOp::Sub)),
        other => panic!("expected Sub unchanged, got {other:?}"),
    }
}

//! End-to-end tests: vyre's self-hosted optimizer passes running as
//! vyre Programs on real CUDA hardware via `CudaBackend::dispatch`.
//!
//! Mirrors the wgpu E2E suites (DCE, const-fold, canonicalize,
//! pipeline). Confirms the OptimizerDispatcher abstraction is
//! backend-agnostic  -  the same encoder + analysis Programs run
//! unchanged on both CUDA and wgpu paths.

#![cfg(test)]

mod common;

use common::{live_backend, CudaOptimizerDispatcher};
use vyre::ir::{BinOp, Expr, Node, Program};
use vyre_foundation::optimizer::fingerprint_program;
use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as cpu_dce_oracle;
use vyre_self_substrate::optimizer::canonicalize_via_encoded::gpu_canonicalize;
use vyre_self_substrate::optimizer::const_fold_via_encoded::gpu_const_fold;
use vyre_self_substrate::optimizer::dce_via_encoded::gpu_dce;

fn wrapped(entry: Vec<Node>) -> Program {
    Program::wrapped(Vec::new(), [1, 1, 1], entry)
}

fn first_let_value(p: &Program) -> Expr {
    match p.entry() {
        [Node::Region { body, .. }] => match body.as_slice() {
            [Node::Let { value, .. }] => value.clone(),
            _ => panic!("expected single Let in body, got {:?}", body),
        },
        _ => panic!("expected wrapped Program with single Region"),
    }
}

// ---- DCE on CUDA -----------------------------------------------------------

fn assert_dce_matches_cpu_oracle_cuda(entry: Vec<Node>) {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let oracle_in = wrapped(entry.clone());
    let test_in = wrapped(entry);

    let oracle_out = cpu_dce_oracle(oracle_in);
    let gpu_out = gpu_dce(test_in, &dispatcher).expect("gpu_dce dispatches through cuda cleanly");
    assert_eq!(
        fingerprint_program(&oracle_out),
        fingerprint_program(&gpu_out),
        "CUDA-dispatched DCE must match the foundation CPU oracle. \
         oracle entry={:?} gpu entry={:?}",
        oracle_out.entry(),
        gpu_out.entry()
    );
}

#[test]
fn cuda_dce_dead_let_dropped() {
    assert_dce_matches_cpu_oracle_cuda(vec![Node::let_bind("dead", Expr::u32(7))]);
}

#[test]
fn cuda_dce_live_let_kept() {
    assert_dce_matches_cpu_oracle_cuda(vec![
        Node::let_bind("x", Expr::u32(7)),
        Node::store("buf", Expr::u32(0), Expr::var("x")),
    ]);
}

#[test]
fn cuda_dce_chained_lets_propagate() {
    assert_dce_matches_cpu_oracle_cuda(vec![
        Node::let_bind("a", Expr::u32(1)),
        Node::let_bind("b", Expr::var("a")),
        Node::store("buf", Expr::u32(0), Expr::var("b")),
    ]);
}

#[test]
fn cuda_dce_loop_with_induction_var() {
    assert_dce_matches_cpu_oracle_cuda(vec![Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(10),
        vec![Node::store("buf", Expr::var("i"), Expr::u32(0))],
    )]);
}

// ---- Const-fold on CUDA ----------------------------------------------------

#[test]
fn cuda_const_fold_two_plus_three_yields_lit_five() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(2), Expr::u32(3)),
    )]);
    let folded = gpu_const_fold(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&folded);
    assert!(
        matches!(got, Expr::LitU32(5)),
        "CUDA const-fold must compute 2 + 3 = 5; got {got:?}"
    );
}

#[test]
fn cuda_const_fold_chained_arithmetic() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::add(Expr::u32(2), Expr::u32(3)), Expr::u32(4)),
    )]);
    let folded = gpu_const_fold(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&folded);
    assert!(matches!(got, Expr::LitU32(20)));
}

#[test]
fn cuda_const_fold_bitwise_ops() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::bitand(
            Expr::bitor(Expr::u32(0xFF), Expr::u32(0x100)),
            Expr::u32(0x1FF),
        ),
    )]);
    let folded = gpu_const_fold(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&folded);
    assert!(matches!(got, Expr::LitU32(0x1FF)));
}

#[test]
fn cuda_const_fold_unfoldable_var_passes_through() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::var("a"), Expr::u32(2)),
    )]);
    let folded = gpu_const_fold(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&folded);
    match got {
        Expr::BinOp { op, .. } => assert!(matches!(op, BinOp::Add)),
        other => panic!("expected unchanged Add; got {other:?}"),
    }
}

// ---- Canonicalize on CUDA --------------------------------------------------

#[test]
fn cuda_canonicalize_lit_plus_var_swaps_to_var_plus_lit() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(1), Expr::var("a")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Add));
            assert!(matches!(*left, Expr::Var(ref n) if n.as_str() == "a"));
            assert!(matches!(*right, Expr::LitU32(1)));
        }
        other => panic!("expected BinOp Add, got {other:?}"),
    }
}

#[test]
fn cuda_canonicalize_var_plus_lit_unchanged() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::var("a"), Expr::u32(1)),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { left, right, .. } => {
            assert!(matches!(*left, Expr::Var(ref n) if n.as_str() == "a"));
            assert!(matches!(*right, Expr::LitU32(1)));
        }
        other => panic!("expected unchanged BinOp Add, got {other:?}"),
    }
}

#[test]
fn cuda_canonicalize_non_commutative_div_unchanged() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::div(Expr::u32(10), Expr::var("a")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Div));
            assert!(matches!(*left, Expr::LitU32(10)));
            assert!(matches!(*right, Expr::Var(ref n) if n.as_str() == "a"));
        }
        other => panic!("expected BinOp Div unchanged, got {other:?}"),
    }
}

// ---- Multi-pass pipeline on CUDA ------------------------------------------

#[test]
fn cuda_full_pipeline_canonicalize_then_const_fold_then_dce() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![
        Node::let_bind("dead", Expr::u32(99)),
        Node::let_bind("live", Expr::add(Expr::u32(1), Expr::u32(2))),
        Node::store(
            "buf",
            Expr::u32(0),
            Expr::add(Expr::u32(3), Expr::var("live")),
        ),
    ]);

    let p = gpu_canonicalize(p, &dispatcher).expect("canonicalize dispatches");
    let p = gpu_const_fold(p, &dispatcher).expect("const-fold dispatches");
    let p = gpu_dce(p, &dispatcher).expect("dce dispatches");

    let body: Vec<Node> = match p.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    assert_eq!(body.len(), 2, "DCE must drop the dead let. Got {body:?}");
    match &body[0] {
        Node::Let { name, value } => {
            assert_eq!(name.as_str(), "live");
            assert!(matches!(value, Expr::LitU32(3)));
        }
        other => panic!("expected `let live = 3`, got {other:?}"),
    }
    match &body[1] {
        Node::Store { value, .. } => match value {
            Expr::BinOp { op, left, right } => {
                assert!(matches!(op, BinOp::Add));
                assert!(matches!(left.as_ref(), Expr::Var(n) if n.as_str() == "live"));
                assert!(matches!(right.as_ref(), Expr::LitU32(3)));
            }
            other => panic!("expected BinOp Add, got {other:?}"),
        },
        other => panic!("expected Store, got {other:?}"),
    }
}

#[test]
fn cuda_pipeline_collapses_unused_compute_chain() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher { backend: &backend };

    let p = wrapped(vec![
        Node::let_bind("a", Expr::add(Expr::u32(5), Expr::u32(7))),
        Node::let_bind("b", Expr::mul(Expr::var("a"), Expr::u32(2))),
        Node::let_bind("c", Expr::sub(Expr::var("b"), Expr::u32(4))),
        Node::store("buf", Expr::u32(0), Expr::add(Expr::var("a"), Expr::u32(1))),
    ]);

    let p = gpu_canonicalize(p, &dispatcher).expect("canonicalize dispatches");
    let p = gpu_const_fold(p, &dispatcher).expect("const-fold dispatches");
    let p = gpu_dce(p, &dispatcher).expect("dce dispatches");

    let body: Vec<Node> = match p.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    assert_eq!(body.len(), 2);
    match &body[0] {
        Node::Let { name, value } => {
            assert_eq!(name.as_str(), "a");
            assert!(matches!(value, Expr::LitU32(12)));
        }
        other => panic!("expected `let a = 12`, got {other:?}"),
    }
}

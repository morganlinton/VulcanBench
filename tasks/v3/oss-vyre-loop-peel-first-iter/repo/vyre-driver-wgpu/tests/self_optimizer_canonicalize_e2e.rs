//! End-to-end test: vyre's canonicalize pass running as a vyre Program
//! on the GPU. The kernel marks each commutative `BinOp` whose
//! operands are (literal, non-literal) for swap; the decoder applies.
//!
//! V1 covers the load-bearing rewrite (literal-on-right). The
//! non-literal sort tie-break and `x == x` self-fold migrate as
//! follow-up kernels.

#![cfg(test)]

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BinOp, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_self_substrate::optimizer::canonicalize_via_encoded::gpu_canonicalize;
use vyre_self_substrate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

struct WgpuOptimizerDispatcher<'a> {
    backend: &'a WgpuBackend,
}

impl<'a> OptimizerDispatcher for WgpuOptimizerDispatcher<'a> {
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[Vec<u8>],
        grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let mut config = DispatchConfig::default();
        config.grid_override = grid_override;
        VyreBackend::dispatch(self.backend, program, inputs, &config)
            .map_err(|err| DispatchError::BackendError(err.to_string()))
    }
}

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

#[test]
fn canonicalize_lit_plus_var_swaps_to_var_plus_lit_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // let x = 1 + a   →   let x = a + 1   (literal on right)
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(1), Expr::var("a")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Add));
            assert!(
                matches!(*left, Expr::Var(ref n) if n.as_str() == "a"),
                "left must be Var(a) after canonicalize, got {left:?}"
            );
            assert!(
                matches!(*right, Expr::LitU32(1)),
                "right must be LitU32(1), got {right:?}"
            );
        }
        other => panic!("expected BinOp Add, got {other:?}"),
    }
}

#[test]
fn canonicalize_var_plus_lit_unchanged_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // let x = a + 1   →   unchanged (already canonical)
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
fn canonicalize_two_lits_unchanged_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // Both literals  -  no swap (CPU canonicalize also leaves these
    // alone for non-tie-breaking ops).
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(2), Expr::u32(3)),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { left, right, .. } => {
            assert!(matches!(*left, Expr::LitU32(2)));
            assert!(matches!(*right, Expr::LitU32(3)));
        }
        other => panic!("expected unchanged BinOp, got {other:?}"),
    }
}

#[test]
fn canonicalize_two_vars_unchanged_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // V1 doesn't tie-break non-literals → unchanged.
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::var("a"), Expr::var("b")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { left, right, .. } => {
            assert!(matches!(*left, Expr::Var(ref n) if n.as_str() == "a"));
            assert!(matches!(*right, Expr::Var(ref n) if n.as_str() == "b"));
        }
        other => panic!("expected unchanged BinOp, got {other:?}"),
    }
}

#[test]
fn canonicalize_lit_times_var_swaps_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // let x = 5 * a   →   let x = a * 5
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::u32(5), Expr::var("a")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Mul));
            assert!(matches!(*left, Expr::Var(ref n) if n.as_str() == "a"));
            assert!(matches!(*right, Expr::LitU32(5)));
        }
        other => panic!("expected BinOp Mul, got {other:?}"),
    }
}

#[test]
fn canonicalize_non_commutative_div_unchanged_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // Div is NOT commutative  -  must NEVER swap regardless of operands.
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::div(Expr::u32(10), Expr::var("a")),
    )]);
    let canon = gpu_canonicalize(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&canon);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Div));
            // Left stays literal (not swapped  -  Div is non-commutative).
            assert!(matches!(*left, Expr::LitU32(10)));
            assert!(matches!(*right, Expr::Var(ref n) if n.as_str() == "a"));
        }
        other => panic!("expected BinOp Div unchanged, got {other:?}"),
    }
}

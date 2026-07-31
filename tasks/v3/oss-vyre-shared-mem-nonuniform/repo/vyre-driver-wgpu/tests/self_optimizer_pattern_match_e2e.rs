//! E2E test: hardcoded algebraic-identity pattern-match running as a
//! vyre Program on wgpu. Each rewrite rule is encoded directly into
//! the kernel's per-Expr branch ladder. V2 will source the rule bank
//! from data buffers; the kernel structure stays identical.

#![cfg(test)]

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BinOp, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_self_substrate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_self_substrate::optimizer::pattern_match_via_encoded::gpu_algebraic_identities;

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
            _ => panic!("expected single Let in body"),
        },
        _ => panic!("expected wrapped Program"),
    }
}

#[test]
fn add_zero_left_collapses_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(0), Expr::var("a")),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(
        matches!(got, Expr::Var(ref n) if n.as_str() == "a"),
        "0 + a → a; got {got:?}"
    );
}

#[test]
fn add_zero_right_collapses_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::var("a"), Expr::u32(0)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::Var(ref n) if n.as_str() == "a"));
}

#[test]
fn mul_one_collapses_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::u32(1), Expr::var("a")),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::Var(ref n) if n.as_str() == "a"));
}

#[test]
fn mul_zero_absorbs_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::u32(0), Expr::var("a")),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::LitU32(0)), "0 * a → 0; got {got:?}");
}

#[test]
fn mul_zero_right_absorbs_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::mul(Expr::var("a"), Expr::u32(0)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    assert!(matches!(got, Expr::LitU32(0)));
}

#[test]
fn unrelated_binop_stays_on_real_gpu() {
    // a - 1 has no algebraic identity; must pass through unchanged.
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::sub(Expr::var("a"), Expr::u32(1)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    match got {
        Expr::BinOp { op, left, right } => {
            assert!(matches!(op, BinOp::Sub));
            assert!(matches!(*left, Expr::Var(ref n) if n.as_str() == "a"));
            assert!(matches!(*right, Expr::LitU32(1)));
        }
        other => panic!("expected unchanged Sub, got {other:?}"),
    }
}

#[test]
fn add_two_lits_unchanged_on_real_gpu() {
    // No identity rule for both-literal  -  patterns require a
    // matching-zero literal alongside a non-literal. (Actual folding
    // is the const-fold pass's job.)
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };
    let p = wrapped(vec![Node::let_bind(
        "x",
        Expr::add(Expr::u32(2), Expr::u32(3)),
    )]);
    let after = gpu_algebraic_identities(p, &dispatcher).expect("dispatches");
    let got = first_let_value(&after);
    // (Add 2 3)  -  left is LitU32(2), so the (Add 0 ?x) pattern
    // doesn't fire (l_val != 0). Should pass through.
    match got {
        Expr::BinOp { op, .. } => assert!(matches!(op, BinOp::Add)),
        other => panic!("expected unchanged BinOp, got {other:?}"),
    }
}

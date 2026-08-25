//! Multi-pass self-hosted optimizer pipeline running entirely on GPU.
//!
//! Composes `gpu_canonicalize → gpu_const_fold → gpu_dce` against the
//! same input Program through `WgpuBackend::dispatch`. Each pass
//! re-encodes its input and dispatches its own analysis Program. No
//! CPU optimizer pass runs at any point.

#![cfg(test)]

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BinOp, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_self_substrate::optimizer::canonicalize_via_encoded::gpu_canonicalize;
use vyre_self_substrate::optimizer::const_fold_via_encoded::gpu_const_fold;
use vyre_self_substrate::optimizer::dce_via_encoded::gpu_dce;
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

#[test]
fn full_pipeline_canonicalize_then_const_fold_then_dce_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // Input program:
    //   let dead = 99;          (dead  -  no use)
    //   let live = 1 + 2;       (foldable to 3)
    //   store buf 0 (3 + live); (canonical: live + 3, then folds: literal hoisted but Var)
    //                           → after canonical: (live + 3) but live is the actual Var,
    //                              so literal-on-right rewrites to (Var(live) + LitU32(3))
    //                              const-fold can't fold this because Var(live) isn't a literal.
    //
    // After canonicalize: `1 + 2` is two literals → unchanged; `3 + live` → `live + 3`
    // After const-fold:   `1 + 2 = 3`, so `let live = 3`; `live + 3` stays (Var not foldable)
    // After DCE:          `let dead = 99` dropped (no use); rest stays
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

    // Expected after all three passes:
    //   let live = 3
    //   store buf 0 (live + 3)    // canonicalize swapped, const-fold left as-is
    assert_eq!(body.len(), 2, "DCE should drop the dead let. Got {body:?}");

    match &body[0] {
        Node::Let { name, value } => {
            assert_eq!(name.as_str(), "live");
            assert!(
                matches!(value, Expr::LitU32(3)),
                "const-fold should compute 1+2=3 → LitU32(3); got {value:?}"
            );
        }
        other => panic!("expected first kept Node to be `let live = 3`, got {other:?}"),
    }

    match &body[1] {
        Node::Store { value, .. } => match value {
            Expr::BinOp { op, left, right } => {
                assert!(matches!(op, BinOp::Add));
                assert!(
                    matches!(left.as_ref(), Expr::Var(n) if n.as_str() == "live"),
                    "canonicalize put Var(live) on the left, got {left:?}"
                );
                assert!(
                    matches!(right.as_ref(), Expr::LitU32(3)),
                    "literal hoisted to right after canonicalize, got {right:?}"
                );
            }
            other => panic!("expected BinOp Add, got {other:?}"),
        },
        other => panic!("expected Store as second kept node, got {other:?}"),
    }
}

#[test]
fn pipeline_collapses_unused_compute_chain_on_real_gpu() {
    let backend = live_backend();
    let dispatcher = WgpuOptimizerDispatcher { backend: &backend };

    // let a = 5 + 7;          (foldable to 12)
    // let b = a * 2;          (foldable to 24)
    // let c = b - 4;          (foldable to 20  -  but unused)
    // store buf 0 (a + 1);    (foldable to 13  -  only a is needed downstream)
    //
    // After canonicalize: `5+7`, `a*2`, `b-4`, `a+1`  -  `a*2` and `a+1` already
    //                     have Var on the left, so unchanged.
    // After const-fold:   a=12, b=24, c=20, but the store has Var(a) so the
    //                     `a+1` BinOp can't fold (a is a Var, not a literal yet).
    //                     The let bindings DO fold their values though.
    // After DCE:          `let c` dropped (unused). `let b` dropped (unused
    //                      -  no Var(b) anywhere reachable). `let a` kept (used
    //                     by the store).
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

    // Expected after the pipeline:
    //   let a = 12
    //   store buf 0 (a + 1)
    assert_eq!(
        body.len(),
        2,
        "DCE should drop both unused lets b and c. Got {body:?}"
    );
    match &body[0] {
        Node::Let { name, value } => {
            assert_eq!(name.as_str(), "a");
            assert!(matches!(value, Expr::LitU32(12)), "got {value:?}");
        }
        other => panic!("expected `let a = 12`, got {other:?}"),
    }
}

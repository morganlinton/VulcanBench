//! End-to-end test: GPU validator on real CUDA hardware.
//!
//! Builds Programs that pass / fail each migrated check and asserts
//! the GPU returns the right verdict.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::validate_via_encoded::{
    gpu_validate_limits, DEFAULT_MAX_EXPR_DEPTH,
};

#[test]
fn cuda_validate_clean_program_no_violations() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("b", Expr::mul(Expr::var("a"), Expr::u32(3))),
            Node::store("buf", Expr::u32(0), Expr::var("b")),
        ],
    );

    let [v033, v019] = gpu_validate_limits(&p, &dispatcher).expect("validate must succeed");
    assert!(!v033, "small clean program must not trip V033 (expr depth)");
    assert!(!v019, "small clean program must not trip V019 (node count)");
}

#[test]
fn cuda_validate_empty_entry_passes() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    let p = Program::wrapped(Vec::new(), [1, 1, 1], vec![]);

    let [v033, v019] = gpu_validate_limits(&p, &dispatcher).expect("validate must succeed");
    assert!(!v033);
    assert!(!v019);
}

#[test]
fn cuda_validate_v033_triggers_when_depth_exceeds_limit() {
    // Build an Expr with nesting depth > DEFAULT_MAX_EXPR_DEPTH
    // (1024). Stack 1100 nested adds: ((((1+1)+1)+1)…)
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    let depth = (DEFAULT_MAX_EXPR_DEPTH as usize) + 50;
    let mut e = Expr::u32(1);
    for _ in 0..depth {
        e = Expr::add(e, Expr::u32(1));
    }
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store("buf", Expr::u32(0), e)],
    );

    let [v033, v019] = gpu_validate_limits(&p, &dispatcher).expect("validate must succeed");
    assert!(
        v033,
        "deeply-nested Expr (depth {} > limit {}) must trip V033",
        depth + 1,
        DEFAULT_MAX_EXPR_DEPTH
    );
    assert!(!v019, "small node count must not trip V019");
}

#[test]
fn cuda_validate_below_depth_limit_passes() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // Build an Expr with depth ≈ 800  -  well under the 1024 limit.
    let depth: usize = 800;
    let mut e = Expr::u32(1);
    for _ in 0..depth {
        e = Expr::add(e, Expr::u32(1));
    }
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store("buf", Expr::u32(0), e)],
    );

    let [v033, _] = gpu_validate_limits(&p, &dispatcher).expect("validate must succeed");
    assert!(
        !v033,
        "Expr depth {} below limit {} must NOT trip V033",
        depth + 1,
        DEFAULT_MAX_EXPR_DEPTH
    );
}

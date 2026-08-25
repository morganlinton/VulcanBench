//! Integration test for the CUDA backend.

mod common;
use common::u32_bytes;
use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    )
}

#[test]
fn zero_workgroup_dimension_is_rejected_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let mut config = DispatchConfig::default();
    config.workgroup_override = Some([0, 1, 1]);

    let err = backend
        .dispatch(&program(), &[u32_bytes(&[1])], &config)
        .expect_err("Fix: zero CUDA workgroup dimension must be rejected.");
    assert!(
        err.to_string().contains("non-zero"),
        "Fix: geometry errors must explain the non-zero dimension contract, got: {err}"
    );
}

#[test]
fn oversize_workgroup_is_rejected_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let mut config = DispatchConfig::default();
    config.workgroup_override = Some([backend.max_threads_per_block() + 1, 1, 1]);

    let err = backend
        .dispatch(&program(), &[u32_bytes(&[1])], &config)
        .expect_err("Fix: oversize CUDA workgroup must be rejected.");
    assert!(
        err.to_string().contains("device max"),
        "Fix: geometry errors must report the device max, got: {err}"
    );
}

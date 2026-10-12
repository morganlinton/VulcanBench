//! Pre-emption / deadline cancellation.
//!
//! See `contracts/release.md`. `DispatchConfig.timeout` must be enforced
//! as a dispatch deadline and leave the GPU in a recoverable state.

use std::time::{Duration, Instant};
use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

/// Build a program the GPU needs at least 2 seconds to finish. The
/// exact shape depends on the vyre IR surface; this shape uses a dense loop
/// per invocation. Keep it calibrated against the CI reference hardware.
fn long_running_program() -> Program {
    const OUTPUT_WORDS: u32 = 16 * 1024 * 1024;
    let mut body = Vec::with_capacity(515);
    body.push(Node::let_bind("idx", Expr::gid_x()));
    body.push(Node::let_bind("acc", Expr::var("idx")));
    for round in 0..512u32 {
        body.push(Node::assign(
            "acc",
            Expr::bitxor(
                Expr::mul(Expr::var("acc"), Expr::u32(1_664_525)),
                Expr::add(
                    Expr::var("idx"),
                    Expr::u32(1_013_904_223u32.wrapping_add(round)),
                ),
            ),
        ));
    }
    body.push(Node::if_then(
        Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
        vec![Node::store("out", Expr::var("idx"), Expr::var("acc"))],
    ));
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)
            .with_count(OUTPUT_WORDS)
            .with_output_byte_range(0..4)],
        [256, 1, 1],
        body,
    )
}

#[test]
fn dispatch_cancels_within_deadline() {
    let backend = WgpuBackend::acquire().expect("Fix: GPU required for pre-emption test");
    let program = long_running_program();
    let mut config = DispatchConfig::default();
    config.timeout = Some(Duration::from_millis(100));
    config.label = Some("dispatch-preemption".to_string());

    let start = Instant::now();
    let result = backend.dispatch(&program, &[], &config);
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "dispatch preemption: dispatch must return Err on timeout, got Ok"
    );
    // wgpu/Vulkan does not support mid-kernel preemption, so cancellation
    // can only check at queue boundaries. Allow 2s past the 100ms timeout
    // for the in-flight kernel to drain plus the cancellation observation
    // window. True GPU pre-emption is a separate roadmap item; this test
    // verifies the contract that timeout DOES return Err in bounded
    // wall-clock, not that it kills the kernel mid-execution.
    assert!(
        elapsed < Duration::from_secs(2),
        "dispatch preemption: cancellation must complete within 2s of the deadline; took {:?}",
        elapsed
    );

    // After cancellation the device must accept a fresh dispatch.
    let quick = vyre::Program::empty();
    let _ = backend
        .dispatch(&quick, &[], &DispatchConfig::default())
        .expect("Fix: device must be usable after cancelled dispatch");
}

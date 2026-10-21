//! End-to-end contract for GPU-side `Node::Trap` propagation.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

#[test]
fn dispatch_reports_node_trap_tag_and_address() {
    assert!(
        !vyre_driver_wgpu::runtime::device::enumerate_adapters().is_empty(),
        "Fix: trap propagation requires a live GPU adapter; adapter discovery returned none."
    );
    let backend =
        WgpuBackend::acquire().expect("Fix: trap propagation must acquire the live WGPU backend.");
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::trap(Expr::u32(7), "trap-propagation-test"),
            Node::store("out", Expr::u32(0), Expr::u32(1)),
        ],
    );

    let err = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect_err("Fix: GPU trap must propagate as a backend error, not a successful output.");
    let message = err.to_string();
    assert!(
        message.contains("trap-propagation-test")
            && message.contains("address=7")
            && message.contains("lane"),
        "Fix: propagated trap error must include the trap tag, address, and lane. Got: {message}",
    );
}

#[test]
fn dispatch_async_reports_node_trap_tag_and_address() {
    assert!(
        !vyre_driver_wgpu::runtime::device::enumerate_adapters().is_empty(),
        "Fix: async trap propagation requires a live GPU adapter; adapter discovery returned none."
    );
    let backend = WgpuBackend::acquire()
        .expect("Fix: async trap propagation must acquire the live WGPU backend.");
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(11), "async-trap-propagation-test")],
    );

    let pending = backend
        .dispatch_async(&program, &[], &DispatchConfig::default())
        .expect("Fix: dispatch_async must submit trap programs so await_result can surface traps.");
    let err = pending
        .await_result()
        .expect_err("Fix: async GPU trap must propagate through the pending dispatch handle.");
    let message = err.to_string();
    assert!(
        message.contains("async-trap-propagation-test")
            && message.contains("address=11")
            && message.contains("lane"),
        "Fix: async propagated trap error must include the trap tag, address, and lane. Got: {message}",
    );
}

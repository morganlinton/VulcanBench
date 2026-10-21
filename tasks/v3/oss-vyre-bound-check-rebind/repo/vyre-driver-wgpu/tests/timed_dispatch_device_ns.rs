//! WGPU timed-dispatch telemetry contract.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

#[test]
fn timed_dispatch_reports_structured_gpu_device_ns() {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: timed WGPU telemetry contract requires a live GPU backend.");
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(4)],
        [32, 1, 1],
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(4)),
                vec![Node::store(
                    "out",
                    Expr::var("idx"),
                    Expr::add(Expr::var("idx"), Expr::u32(7)),
                )],
            ),
        ],
    );

    let timed = backend
        .dispatch_borrowed_timed(&program, &[], &DispatchConfig::default())
        .expect("Fix: WGPU timed dispatch should execute through GPU timestamp telemetry.");

    assert_eq!(timed.outputs.len(), 1);
    assert_eq!(timed.outputs[0].len(), 16);
    assert!(
        timed.device_ns.unwrap_or_default() > 0,
        "Fix: WGPU timed dispatch must expose structured GPU device_ns, not trace-only timestamp logs."
    );
}

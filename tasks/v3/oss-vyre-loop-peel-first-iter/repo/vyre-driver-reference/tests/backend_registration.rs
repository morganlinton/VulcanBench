//! Registration contract for the pure Rust reference backend adapter.

use vyre_driver::backend::{acquire, backend_dispatches};
use vyre_foundation::ir::{Expr, Node, Program};

mod support;
use support::u32_out_buffer;

#[test]
fn cpu_ref_registers_as_dispatch_backend() {
    assert!(
        backend_dispatches(vyre_driver_reference::CPU_REF_BACKEND_ID),
        "Fix: vyre-driver-reference must register cpu-ref as a dispatch-capable backend."
    );

    let backend = acquire(vyre_driver_reference::CPU_REF_BACKEND_ID)
        .expect("Fix: cpu-ref backend registration must construct without host hardware.");
    let program = Program::wrapped(
        vec![u32_out_buffer("out", 0)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );
    let outputs = backend
        .dispatch(&program, &[], &vyre_driver::DispatchConfig::default())
        .expect("Fix: cpu-ref backend must dispatch a minimal Program.");

    assert_eq!(
        outputs,
        vec![42u32.to_le_bytes().to_vec()],
        "Fix: cpu-ref backend output must match reference interpreter bytes."
    );
}

//! Resident timed-dispatch output contract tests for the WGPU backend.

use std::sync::Arc;

use vyre_driver::Resource;
use vyre_driver::VyreBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn resident_timed_dispatch_returns_public_readwrite_outputs() {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU resident-output regression test requires a live GPU backend.");
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
        )],
    );

    let out = backend
        .allocate_resident(4)
        .expect("Fix: WGPU must support resident output allocation.");
    let input = backend
        .allocate_resident(4)
        .expect("Fix: WGPU must support resident input allocation.");
    let result = (|| {
        backend.upload_resident(&out, &[0, 0, 0, 0])?;
        backend.upload_resident(&input, &37u32.to_le_bytes())?;
        let timed = backend.dispatch_resident_timed(
            &program,
            &[out.clone(), input.clone()],
            &vyre_driver::DispatchConfig::default(),
        )?;
        assert_eq!(
            timed.outputs.len(),
            1,
            "resident timed dispatch must return public ReadWrite outputs"
        );
        assert_eq!(timed.outputs[0], 42u32.to_le_bytes());
        assert!(
            timed.device_ns.unwrap_or_default() > 0,
            "Fix: WGPU resident timed dispatch must report GPU timestamp device_ns so release benchmarks do not fall back to readback wall time."
        );
        Ok::<(), vyre_driver::BackendError>(())
    })();
    let free_out = backend.free_resident(out);
    let free_input = backend.free_resident(input);
    result.expect("Fix: resident timed dispatch must execute and read back outputs.");
    free_out.expect("Fix: WGPU resident output cleanup must succeed.");
    free_input.expect("Fix: WGPU resident input cleanup must succeed.");
}

#[test]
fn persistent_resource_output_dispatch_rejects_borrowed_outputs_before_launch() {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU resident-output regression test requires a live GPU backend.");
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::load("input", Expr::u32(0)), Expr::u32(5)),
        )],
    );
    let config = vyre_driver::DispatchConfig::default();
    let pipeline = vyre_driver::pipeline::compile(Arc::new(backend.clone()), &program, &config)
        .expect("Fix: WGPU compiled pipeline creation must succeed for resident outputs.");
    let input = backend
        .allocate_resident(4)
        .expect("Fix: WGPU must support resident input allocation.");
    let result = (|| {
        backend.upload_resident(&input, &37u32.to_le_bytes())?;
        let err = pipeline
            .dispatch_persistent_resource_outputs(
                &[Resource::Borrowed(vec![0; 4]), input.clone()],
                &config,
            )
            .expect_err("Fix: resident-output mode must reject borrowed output resources");
        assert!(
            err.to_string().contains("cannot return borrowed output binding"),
            "borrowed output resource error must explain zero-copy resident-output requirements, got: {err}"
        );
        Ok::<(), vyre_driver::BackendError>(())
    })();
    let free_input = backend.free_resident(input);
    result.expect("Fix: borrowed output rejection must happen before launch.");
    free_input.expect("Fix: WGPU resident input cleanup must succeed.");
}

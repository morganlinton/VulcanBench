//! Integration test for the CUDA backend.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_cuda::{codegen::program_to_ptx, CudaBackend, CudaBackendRegistration};
use vyre_foundation::ir::{BufferDecl, DataType, Node, Program};

fn indirect_dispatch_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read("counts", 0, DataType::U32).with_count(3)],
        [1, 1, 1],
        vec![Node::IndirectDispatch {
            count_buffer: "counts".into(),
            count_offset: 0,
        }],
    )
}

#[test]
fn ptx_lowering_names_unsupported_node_variant() {
    let program = indirect_dispatch_program();
    let err = program_to_ptx(&program, &DispatchConfig::default())
        .expect_err("Fix: CUDA PTX lowering must reject unsupported IndirectDispatch nodes.");
    assert!(
        err.contains("IndirectDispatch"),
        "Fix: unsupported-node errors must name the exact IR variant, got: {err}"
    );
}

#[test]
fn dispatch_rejects_unsupported_capability_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = indirect_dispatch_program();
    let err = backend
        .dispatch(
            &program,
            &[[3u32.to_le_bytes(), 1u32.to_le_bytes(), 1u32.to_le_bytes()].concat()],
            &DispatchConfig::default(),
        )
        .expect_err("Fix: unsupported CUDA IR must fail before launch.");
    let message = err.to_string();
    assert!(
        message.contains("Fix:")
            && message.contains("missing required capabilities")
            && message.contains("indirect_dispatch"),
        "Fix: CUDA dispatch must reject unsupported capabilities before launch, got: {message}"
    );
}

#[test]
fn registration_borrowed_async_rejects_unsupported_capability_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let registration = CudaBackendRegistration::new(backend);
    let program = indirect_dispatch_program();
    let counts = [3u32.to_le_bytes(), 1u32.to_le_bytes(), 1u32.to_le_bytes()].concat();
    let err = expect_backend_error(
        registration.dispatch_borrowed_async(
            &program,
            &[counts.as_slice()],
            &DispatchConfig::default(),
        ),
        "Fix: CUDA registration borrowed async dispatch must fail before launch.",
    );

    assert_missing_indirect_dispatch_capability(err);
}

#[test]
fn registration_compile_native_rejects_unsupported_capability_before_ptx_emit() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let registration = CudaBackendRegistration::new(backend);
    let program = indirect_dispatch_program();
    let err = expect_backend_error(
        registration.compile_native(&program, &DispatchConfig::default()),
        "Fix: CUDA registration compile_native must fail before PTX emission.",
    );

    assert_missing_indirect_dispatch_capability(err);
}

#[test]
fn registration_resident_dispatch_rejects_unsupported_capability_before_launch() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let registration = CudaBackendRegistration::new(backend);
    let program = indirect_dispatch_program();
    let resource = registration
        .allocate_resident(12)
        .expect("Fix: resident allocation failed on a GPU-required host.");

    let err = expect_backend_error(
        registration.dispatch_resident_timed(
            &program,
            &[resource.clone()],
            &DispatchConfig::default(),
        ),
        "Fix: CUDA registration resident dispatch must fail before launch.",
    );
    registration
        .free_resident(resource)
        .expect("Fix: resident free failed after unsupported-capability rejection.");

    assert_missing_indirect_dispatch_capability(err);
}

fn assert_missing_indirect_dispatch_capability(err: vyre_driver::BackendError) {
    let message = err.to_string();
    assert!(
        message.contains("Fix:")
            && message.contains("missing required capabilities")
            && message.contains("indirect_dispatch"),
        "Fix: CUDA registration facade must reject unsupported capabilities before lowering/launch, got: {message}"
    );
}

fn expect_backend_error<T>(
    result: Result<T, vyre_driver::BackendError>,
    message: &'static str,
) -> vyre_driver::BackendError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}

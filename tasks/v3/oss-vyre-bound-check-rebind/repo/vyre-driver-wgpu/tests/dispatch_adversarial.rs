//! P1 inventory #88  -  adversarial tests for every dispatch path.
//!
//! Hostile inputs against `WgpuBackend::dispatch` and friends. The
//! test suite asserts each adversarial input produces a structured
//! `BackendError` rather than a panic, a hang, or undefined behavior.
//!
//! Coverage targets (the 6 dispatch paths the audit calls out):
//!   - direct synchronous dispatch
//!   - compiled-pipeline dispatch
//!   - async dispatch
//!   - compound dispatch (multi-stage)
//!   - persistent dispatch
//!   - megakernel dispatch
//!
//! GPU-required: each test acquires a real adapter; no silent skip.
//! `scripts/check_gpu_test_loudness.sh` enforces the loudness rule.

use vyre::ir::{BufferDecl, DataType, Program};
use vyre::{BackendError, VyreBackend};

fn empty_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32)],
        [1, 1, 1],
        Vec::new(),
    )
}

#[test]
fn empty_program_dispatch_returns_structured_error() {
    // An empty Program (no entry nodes) must NOT crash the wgpu
    // backend; it must return a structured BackendError.
    let program = empty_program();

    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: live WGPU backend is required for empty-program dispatch coverage");

    let inputs: Vec<Vec<u8>> = vec![];
    let config = vyre::DispatchConfig::default();
    let result = backend.dispatch(&program, &inputs, &config);
    assert!(
        matches!(
            result,
            Err(BackendError::InvalidProgram { .. })
                | Err(BackendError::DispatchFailed { .. })
                | Err(BackendError::KernelCompileFailed { .. })
        ),
        "empty program should yield a structured BackendError, got {result:?}"
    );
}

#[test]
fn dispatch_with_mismatched_inputs_yields_structured_error() {
    // Program declares one read buffer; we pass zero inputs  -  the
    // backend must structurally reject before submitting.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        Vec::new(),
    );
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: live WGPU backend is required for adversarial dispatch coverage");
    let inputs: Vec<Vec<u8>> = vec![]; // mismatched: 0 < 1 expected
    let config = vyre::DispatchConfig::default();
    let result = backend.dispatch(&program, &inputs, &config);
    assert!(
        matches!(
            result,
            Err(BackendError::InvalidProgram { .. })
                | Err(BackendError::DispatchFailed { .. })
                | Err(BackendError::KernelCompileFailed { .. })
        ),
        "missing-input dispatch must fail; got {result:?}"
    );
}

#[test]
fn empty_program_compile_native_returns_result() {
    let program = empty_program();
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: live WGPU backend is required for compile_native adversarial coverage");
    // Compiled-pipeline path: `VyreBackend::compile_native` must
    // either return a CompiledPipeline (Ok(Some)), opt out (Ok(None)),
    // or surface a structured BackendError. The failure modes the
    // gate forbids are a panic or undefined behavior.
    let config = vyre::DispatchConfig::default();
    if let Err(error) = backend.compile_native(&program, &config) {
        assert_ne!(
            error.to_string().len(),
            0,
            "structured BackendError must carry diagnostic text"
        );
    }
}

//! WgpuBackend dispatch_async validation-error contract tests.
//!
//! Non-blocking behavior is covered by `async_dispatch_non_blocking`; this
//! file covers the deferred API boundary where validation must fail before a
//! pending GPU handle is returned.

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BufferDecl, DataType, Node, Program};
use vyre::{DispatchConfig, VyreBackend};

#[test]
fn validation_errors_propagate_immediately_from_dispatch_async() {
    let backend = live_backend();

    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "half_out",
            0,
            vyre::ir::BufferAccess::ReadWrite,
            DataType::F16,
        )
        .with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let result = backend.dispatch_async(&program, &[], &DispatchConfig::default());
    assert!(
        result.is_err(),
        "Fix: validation errors inside dispatch_async must propagate immediately"
    );
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    let text = err.to_string();
    assert!(
        text.contains("Fix:"),
        "Fix: validation error from dispatch_async must be actionable. Got: {text}"
    );
}

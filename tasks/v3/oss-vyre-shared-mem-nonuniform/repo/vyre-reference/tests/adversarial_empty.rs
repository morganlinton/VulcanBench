//! Adversarial empty and malformed-boundary coverage for the reference interpreter.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::{reference_eval, value::Value};

#[test]
fn empty_wrapped_program_returns_no_outputs() {
    let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
    let outputs = reference_eval(&program, &[]).expect("Fix: empty wrapped Program must evaluate");
    assert!(outputs.is_empty());
}

#[test]
fn raw_empty_program_is_rejected_with_region_context() {
    #[allow(deprecated)]
    let program = Program::new(Vec::new(), [1, 1, 1], Vec::new());
    let err = reference_eval(&program, &[]).expect_err("Fix: raw empty Program must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("top-level Region"),
        "expected top-level Region diagnostic, got: {message}"
    );
}

#[test]
fn zero_length_input_does_not_create_implicit_bytes() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    );

    let err = reference_eval(&program, &[Value::Bytes(Vec::new().into())])
        .expect_err("Fix: zero-byte input for u32 load must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("input") || message.contains("buffer"),
        "expected actionable buffer diagnostic, got: {message}"
    );
}

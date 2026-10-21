//! Regression tests for V022 validation: at most one output buffer per program.
//!
//! V022 is triggered when a program declares more than one buffer with
//! `BufferAccess::WriteOnly` (i.e. `BufferDecl::output(...)`). Zero or one
//! output buffers are legal; two or more are rejected.

use vyre::ir::{BufferDecl, DataType, Node, Program};
use vyre::validate;

#[test]
fn zero_output_buffers_passes() {
    let program = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V022")),
        "zero output buffers must not trigger V022, got: {:?}",
        errors
    );
}

#[test]
fn single_output_buffer_passes() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        !errors.iter().any(|e| e.message().contains("V022")),
        "single output buffer must not trigger V022, got: {:?}",
        errors
    );
}

#[test]
fn two_output_buffers_fails_with_v022() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out_a", 0, DataType::U32).with_count(1),
            BufferDecl::output("out_b", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e.message().contains("V022")),
        "two output buffers must trigger V022, got: {:?}",
        errors
    );
}

#[test]
fn three_output_buffers_fails_with_v022() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out_a", 0, DataType::U32).with_count(1),
            BufferDecl::output("out_b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out_c", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e.message().contains("V022")),
        "three output buffers must trigger V022, got: {:?}",
        errors
    );
}

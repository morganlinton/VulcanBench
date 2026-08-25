//! Adversarial tests for validator edge cases and boundary conditions.
//!
//! These tests exercise paths that are easy to miss during normal
//! development: empty programs, no-output programs, programs with
//! only a Return node, and other degenerate but legal IR shapes.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::validate;

#[test]
fn program_with_no_buffers_passes_validation() {
    let program = Program::wrapped(vec![], [1, 1, 1], vec![Node::Return]);
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "program with no buffers must validate, got: {:?}",
        errors
    );
}

#[test]
fn program_with_no_output_buffer_passes_validation() {
    let program = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "program with only read buffers (no output) must validate, got: {:?}",
        errors
    );
}

#[test]
fn program_with_only_return_passes_validation() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "program with only Return must validate, got: {:?}",
        errors
    );
}

#[test]
fn program_with_two_output_buffers_is_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out1", 0, DataType::U32).with_count(1),
            BufferDecl::output("out2", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.iter().any(|e| e.message().contains("V022")),
        "program with two output buffers must be rejected with V022, got: {:?}",
        errors
    );
}

#[test]
fn program_with_duplicate_buffer_names_is_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("dup", 0, DataType::U32).with_count(1),
            BufferDecl::read("dup", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("duplicate buffer name")),
        "program with duplicate buffer names must be rejected, got: {:?}",
        errors
    );
}

#[test]
fn program_with_duplicate_bindings_is_rejected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("duplicate binding slot")),
        "program with duplicate binding slots must be rejected, got: {:?}",
        errors
    );
}

#[test]
fn program_with_unbound_store_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::store("undeclared", Expr::u32(0), Expr::u32(42)),
            Node::Return,
        ],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("store to unknown buffer")),
        "program storing to undeclared buffer must be rejected, got: {:?}",
        errors
    );
}

#[test]
fn program_with_unbound_load_is_rejected() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("undeclared", Expr::u32(0))),
            Node::Return,
        ],
    );
    let errors = validate(&program);
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("load from unknown buffer")),
        "program loading from undeclared buffer must be rejected, got: {:?}",
        errors
    );
}

#[test]
fn program_with_self_assignment_passes_validation() {
    // Self-assignment is valid IR (the optimizer may eliminate it).
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::assign("out", Expr::load("out", Expr::u32(0))),
            Node::Return,
        ],
    );
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "program with self-assignment must validate, got: {:?}",
        errors
    );
}

#[test]
fn program_with_unused_let_passes_validation() {
    // Unused bindings are valid IR (the DCE pass may eliminate them).
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("unused", Expr::u32(42)), Node::Return],
    );
    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "program with unused let binding must validate, got: {:?}",
        errors
    );
}

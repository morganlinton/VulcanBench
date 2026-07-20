//! Failure-oriented tests: lowering must produce actionable errors.
//!
//! Every rejection path in the wgpu Naga lowering must contain a
//! `Fix:` hint so authors know what to change. Silent or vague
//! errors are regressions.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::emit;

fn assert_rejects_with_fix(program: &Program, context: &str) {
    let err =
        emit::lower(program).expect_err(&format!("Fix: {context} must reject in wgpu lowering"));
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:"),
        "Fix: {context} rejection must be actionable. Got: {msg}"
    );
}

#[test]
fn f16_buffer_rejection_contains_fix_guidance() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F16)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "F16 buffer");
}

#[test]
fn bytes_buffer_rejection_contains_fix_guidance() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "raw",
            0,
            BufferAccess::ReadWrite,
            DataType::Bytes,
        )],
        [1, 1, 1],
        vec![Node::store("raw", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "Bytes buffer");
}

#[test]
fn f64_buffer_rejection_contains_fix_guidance() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::F64)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "F64 buffer");
}

#[test]
fn bf16_buffer_rejection_contains_fix_guidance() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::BF16)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "BF16 buffer");
}

#[test]
fn i64_buffer_rejection_contains_fix_guidance() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::I64)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "I64 buffer");
}

#[test]
fn persistent_memory_rejection_contains_fix_guidance() {
    let mut buf = BufferDecl::storage("persist", 0, BufferAccess::ReadWrite, DataType::U32);
    buf.kind = vyre::ir::MemoryKind::Persistent;
    let program = Program::wrapped(
        vec![buf],
        [1, 1, 1],
        vec![Node::store("persist", Expr::u32(0), Expr::u32(0))],
    );
    assert_rejects_with_fix(&program, "Persistent memory");
}

#[test]
fn zero_count_static_buffer_rejection_contains_fix_guidance() {
    let mut buf = BufferDecl::storage("local", 0, BufferAccess::ReadWrite, DataType::U32);
    buf.kind = vyre::ir::MemoryKind::Local;
    buf.count = 0;
    let program2 = Program::wrapped(
        vec![buf],
        [1, 1, 1],
        vec![Node::store("local", Expr::u32(0), Expr::u32(1))],
    );
    assert_rejects_with_fix(&program2, "zero-count static buffer");
}

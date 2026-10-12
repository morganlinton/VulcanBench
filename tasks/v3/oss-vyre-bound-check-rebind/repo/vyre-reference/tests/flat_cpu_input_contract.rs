//! Flat CPU adapter input-boundary contracts.
//!
//! The flat adapter is the byte-level CPU oracle used by conformance flows.
//! Truncated fixed-width inputs must fail loudly instead of being zero-padded,
//! and trailing input bytes must fail instead of being ignored. Otherwise
//! malformed vectors can accidentally match backend output.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn copy_input_to_output_program(input: BufferDecl) -> Program {
    Program::wrapped(
        vec![
            input.with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    )
}

#[test]
fn flat_cpu_exact_readonly_input_round_trips_without_padding() {
    let program = copy_input_to_output_program(BufferDecl::read("input", 0, DataType::U32));
    let mut output = Vec::new();

    vyre_reference::flat_cpu::run_flat(&program, &0xAABB_CCDDu32.to_le_bytes(), &mut output)
        .expect("Fix: exact-width flat CPU input must run");

    assert_eq!(output, 0xAABB_CCDDu32.to_le_bytes());
}

#[test]
fn flat_cpu_rejects_truncated_readonly_input() {
    let program = copy_input_to_output_program(BufferDecl::read("input", 0, DataType::U32));
    let mut output = vec![0xEE];

    let error = vyre_reference::flat_cpu::run_flat(&program, &[0xDD, 0xCC, 0xBB], &mut output)
        .expect_err("Fix: flat CPU must reject truncated fixed-width input");
    let message = error.to_string();

    assert!(
        message.contains("truncated") && message.contains("input") && message.contains("4"),
        "expected actionable truncated-input diagnostic, got: {message}"
    );
    assert_eq!(
        output,
        vec![0xEE],
        "failed flat CPU decoding must not clear or partially rewrite caller output"
    );
}

#[test]
fn flat_cpu_rejects_truncated_uniform_input() {
    let program = copy_input_to_output_program(BufferDecl::uniform("input", 0, DataType::U32));
    let mut output = Vec::new();

    let error = vyre_reference::flat_cpu::run_flat(&program, &[0x01, 0x02], &mut output)
        .expect_err("Fix: flat CPU must reject truncated uniform input");
    let message = error.to_string();

    assert!(
        message.contains("truncated") && message.contains("input") && message.contains("2"),
        "expected uniform truncated-input diagnostic, got: {message}"
    );
    assert!(output.is_empty());
}

#[test]
fn flat_cpu_rejects_trailing_input_bytes() {
    let program = copy_input_to_output_program(BufferDecl::read("input", 0, DataType::U32));
    let mut payload = 0x1122_3344u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&[0x55, 0x66]);
    let mut output = vec![0xAA];

    let error = vyre_reference::flat_cpu::run_flat(&program, &payload, &mut output)
        .expect_err("Fix: flat CPU must reject trailing bytes after exact input consumption");
    let message = error.to_string();

    assert!(
        message.contains("trailing") && message.contains("2"),
        "expected actionable trailing-input diagnostic, got: {message}"
    );
    assert_eq!(
        output,
        vec![0xAA],
        "failed flat CPU decoding must not clear or partially rewrite caller output"
    );
}

#[test]
fn flat_cpu_rejects_payload_for_program_without_flat_inputs() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
    );
    let mut output = Vec::new();

    let error = vyre_reference::flat_cpu::run_flat(&program, &[0xFF], &mut output)
        .expect_err("Fix: flat CPU must reject payload bytes when no flat input buffers exist");
    let message = error.to_string();

    assert!(
        message.contains("trailing") && message.contains("1"),
        "expected no-input trailing-byte diagnostic, got: {message}"
    );
    assert!(output.is_empty());
}

#[test]
fn flat_cpu_consumes_declared_input_count() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(2),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(1)),
        )],
    );
    let mut payload = 0x1111_1111u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&0x2222_2222u32.to_le_bytes());
    let mut output = Vec::new();

    vyre_reference::flat_cpu::run_flat(&program, &payload, &mut output)
        .expect("Fix: flat CPU must consume all declared input elements");

    assert_eq!(output, 0x2222_2222u32.to_le_bytes());
}

#[test]
fn flat_cpu_initializes_declared_output_count() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(2)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(1), Expr::u32(0xABCD_EF01))],
    );
    let mut output = Vec::new();

    vyre_reference::flat_cpu::run_flat(&program, &[], &mut output)
        .expect("Fix: flat CPU must allocate all declared output elements");

    let mut expected = 0u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&0xABCD_EF01u32.to_le_bytes());
    assert_eq!(output, expected);
}

#[test]
fn flat_cpu_rejects_variable_width_input_buffers() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::Bytes).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    let mut output = vec![0xAA];

    let error = vyre_reference::flat_cpu::run_flat(&program, &[0, 1, 2, 3], &mut output)
        .expect_err("Fix: flat CPU must reject variable-width input buffers");
    let message = error.to_string();

    assert!(
        message.contains("variable-width") && message.contains("input"),
        "expected variable-width input diagnostic, got: {message}"
    );
    assert_eq!(output, vec![0xAA]);
}

#[test]
fn flat_cpu_rejects_variable_width_output_buffers() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::Tensor).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );
    let mut output = Vec::new();

    let error = vyre_reference::flat_cpu::run_flat(&program, &[], &mut output)
        .expect_err("Fix: flat CPU must reject variable-width output buffers");
    let message = error.to_string();

    assert!(
        message.contains("variable-width") && message.contains("out"),
        "expected variable-width output diagnostic, got: {message}"
    );
    assert!(output.is_empty());
}

#[test]
fn flat_cpu_returns_write_only_output_buffers() {
    let program = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::WriteOnly, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0xCAFE_BABE))],
    );
    let mut output = Vec::new();

    vyre_reference::flat_cpu::run_flat(&program, &[], &mut output)
        .expect("Fix: flat CPU must treat WriteOnly storage as backend-allocated output");

    assert_eq!(output, 0xCAFE_BABEu32.to_le_bytes());
}

//! Test crate.

#![cfg(feature = "logical")]
#![allow(deprecated)]
use vyre_reference::value::Value;

fn assert_size_mismatch_is_result_error(program: vyre::Program) {
    let error = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(vec![0u8; 16]),
            Value::from(vec![0u8; 16]),
            Value::from(vec![0u8; 12]),
        ],
    )
    .expect_err("Fix: output buffer size mismatch must return a reference_eval error");
    let message = error.to_string();
    assert!(
        message.contains("out") || message.contains("buffer"),
        "Fix: logical buffer-size errors must name the buffer contract: {message}"
    );
}

#[test]
fn and_errors_on_output_buffer_size_mismatch() {
    assert_size_mismatch_is_result_error(vyre_libs::logical::and("a", "b", "out", 4));
}

#[test]
fn or_errors_on_output_buffer_size_mismatch() {
    assert_size_mismatch_is_result_error(vyre_libs::logical::or("a", "b", "out", 4));
}

#[test]
fn xor_errors_on_output_buffer_size_mismatch() {
    assert_size_mismatch_is_result_error(vyre_libs::logical::xor("a", "b", "out", 4));
}

#[test]
fn nand_errors_on_output_buffer_size_mismatch() {
    assert_size_mismatch_is_result_error(vyre_libs::logical::nand("a", "b", "out", 4));
}

#[test]
fn nor_errors_on_output_buffer_size_mismatch() {
    assert_size_mismatch_is_result_error(vyre_libs::logical::nor("a", "b", "out", 4));
}

//! Test crate.

#![cfg(feature = "crypto-blake3")]
#![allow(deprecated)]
use vyre_reference::value::Value;

fn run_bad_case(cv_in: Vec<u8>, msg: Vec<u8>) {
    let program = vyre_libs::hash::blake3_compress("cv_in", "msg", "params", "cv_out");
    let params = [0u32, 0, 64, 0b0000_1011]
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let error = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(cv_in),
            Value::from(msg),
            Value::from(params),
            Value::from(vec![0u8; 32]),
        ],
    )
    .expect_err("Fix: malformed BLAKE3 buffer size must return a reference_eval error");
    let message = error.to_string();
    assert!(
        message.contains("buffer") || message.contains("load"),
        "Fix: BLAKE3 malformed-buffer error must identify the buffer/load contract: {message}"
    );
}

#[test]
fn blake3_compress_errors_on_wrong_chaining_value_words() {
    run_bad_case(vec![0u8; 7 * 4], vec![0u8; 16 * 4]);
}

#[test]
fn blake3_compress_errors_on_wrong_message_block_words() {
    run_bad_case(vec![0u8; 8 * 4], vec![0u8; 15 * 4]);
}

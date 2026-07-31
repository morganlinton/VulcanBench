//! Parity test for vyre-primitives text::encoding_classify against
//! the histogram-based CPU classifier oracle.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::text::encoding_classify::{
    classify_from_histogram, encoding_classify, ENCODING_CLASSIFY_WORKGROUP_SIZE, ENC_ASCII,
    ENC_BINARY, ENC_ISO8859_1, ENC_UTF16LE, ENC_UTF8,
};

fn run_classify(histogram: &[u32; 256], count: u32) -> u32 {
    let program = encoding_classify("histogram", "encoding", count);
    assert_eq!(program.workgroup_size(), ENCODING_CLASSIFY_WORKGROUP_SIZE);
    // Output buffer is declared via BufferDecl::output, so it does not
    // consume an input slot.
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(histogram)];
    let mut config = DispatchConfig::default();
    // Single-result classifier; invocation 0 writes the output.
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("encoding classify", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA encoding classify dispatch failed: {error}"))
    });
    bytes_u32(&outputs[0])[0]
}

#[test]
fn cuda_encoding_classify_ascii() {
    let mut histogram = [0u32; 256];
    histogram[usize::from(b'H')] = 1;
    histogram[usize::from(b'e')] = 1;
    histogram[usize::from(b'l')] = 2;
    histogram[usize::from(b'o')] = 1;
    let count = 5u32;
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_ASCII);
}

#[test]
fn cuda_encoding_classify_utf8_two_byte() {
    let mut histogram = [0u32; 256];
    // Two copies of "é" = 0xC3 0xA9.
    histogram[0xC3] = 2;
    histogram[0xA9] = 2;
    let count = 4u32;
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_UTF8);
}

#[test]
fn cuda_encoding_classify_utf16le_via_null_density() {
    // 16 bytes with 4 NULs (>1/8 of count) → UTF16LE.
    let mut histogram = [0u32; 256];
    histogram[0x00] = 4;
    histogram[b'h' as usize] = 4;
    histogram[b'i' as usize] = 4;
    histogram[b'!' as usize] = 4;
    let count = 16u32;
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_UTF16LE);
}

#[test]
fn cuda_encoding_classify_iso8859_1_unbalanced_starters() {
    // 5 starter_2 bytes (0xC3) but zero continuation bytes  -  UTF-8
    // would require ~5 continuations to follow them. The mismatch
    // exceeds tolerance so the classifier falls back to ISO-8859-1.
    let mut histogram = [0u32; 256];
    for _ in 0..10 {
        histogram[b'a' as usize] += 1;
    }
    histogram[0xC3] = 5;
    let count = 15u32;
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_ISO8859_1);
}

#[test]
fn cuda_encoding_classify_zero_count_is_ascii() {
    let histogram = [0u32; 256];
    let cpu = classify_from_histogram(&histogram, 0);
    let gpu = run_classify(&histogram, 0);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_ASCII);
}

#[test]
fn cuda_encoding_classify_rejects_wrapped_three_byte_shape_count() {
    let mut histogram = [0u32; 256];
    histogram[0xE0] = u32::MAX / 2 + 1;
    let count = histogram[0xE0];
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_ISO8859_1);
}

#[test]
fn cuda_encoding_classify_rejects_wrapped_four_byte_shape_count() {
    let mut histogram = [0u32; 256];
    histogram[0x80] = 2;
    histogram[0xF0] = u32::MAX / 3 + 1;
    let count = histogram[0x80] + histogram[0xF0];
    let cpu = classify_from_histogram(&histogram, count);
    let gpu = run_classify(&histogram, count);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, ENC_ISO8859_1);
}

#[test]
fn cuda_encoding_classify_constants_round_trip() {
    // Sanity: ENC_BINARY is 255, the unknown sentinel.
    assert_eq!(ENC_BINARY, 255);
}

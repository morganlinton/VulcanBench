//! Parity test: vyre-primitives byte_histogram_256 + utf8_shape_counts
//! match their reference oracles.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::ir::{BufferAccess, DataType, Program};
use vyre::DispatchConfig;
use vyre_primitives::text::byte_histogram::{
    byte_histogram_256, byte_histogram_256_u8, reference_byte_histogram,
};
use vyre_primitives::text::utf8_shape_counts::{reference_utf8_shape_counts, utf8_shape_counts};

fn bytes_to_u32_per_lane(source: &[u8]) -> Vec<u32> {
    source.iter().map(|&b| b as u32).collect()
}

fn inputs_for_histogram_program(program: &Program, source: &[u8]) -> Vec<Vec<u8>> {
    program
        .buffers()
        .iter()
        .filter_map(|buffer| {
            let needs_input = matches!(
                buffer.access(),
                BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
            ) && !buffer.is_output()
                && !buffer.is_pipeline_live_out()
                && buffer.access() != BufferAccess::Workgroup;
            if !needs_input {
                return None;
            }
            if buffer.name() == "source" {
                match buffer.element() {
                    DataType::U8 => Some(source.to_vec()),
                    DataType::U32 => {
                        let mut words = bytes_to_u32_per_lane(source);
                        words.resize(buffer.count().max(source.len() as u32) as usize, 0);
                        Some(u32_bytes(&words))
                    }
                    other => {
                        panic!("Fix: CUDA byte histogram source must be U8 or U32, got {other:?}")
                    }
                }
            } else {
                Some(vec![
                    0u8;
                    buffer.count().max(1) as usize
                        * buffer.element().min_bytes()
                ])
            }
        })
        .collect()
}

fn run_histogram_program(program: Program, source: &[u8], case_name: &str) -> Vec<u32> {
    let inputs = inputs_for_histogram_program(&program, source);
    let mut config = DispatchConfig::default();
    // Histogram kernel: 256 lanes per workgroup (256 buckets).
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend(case_name, |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA {case_name} dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(256);
    out
}

fn run_histogram(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = byte_histogram_256("source", "histogram", n);
    run_histogram_program(program, source, "byte histogram")
}

fn run_histogram_u8(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = byte_histogram_256_u8("source", "histogram", n);
    run_histogram_program(program, source, "packed-u8 byte histogram")
}

#[test]
fn cuda_byte_histogram_simple() {
    let source = b"abacab";
    let cpu = reference_byte_histogram(source);
    let gpu = run_histogram(source);
    let gpu_u8 = run_histogram_u8(source);
    assert_eq!(gpu, cpu.to_vec());
    assert_eq!(gpu_u8, cpu.to_vec());
    assert_eq!(gpu[b'a' as usize], 3);
    assert_eq!(gpu[b'b' as usize], 2);
    assert_eq!(gpu[b'c' as usize], 1);
}

#[test]
fn cuda_byte_histogram_utf8_bytes() {
    // 'é' = 0xC3 0xA9
    let source = &[b'a', b'b', b'a', 0xC3, 0xA9];
    let cpu = reference_byte_histogram(source);
    let gpu = run_histogram(source);
    let gpu_u8 = run_histogram_u8(source);
    assert_eq!(gpu, cpu.to_vec());
    assert_eq!(gpu_u8, cpu.to_vec());
    assert_eq!(gpu[b'a' as usize], 2);
    assert_eq!(gpu[0xC3], 1);
    assert_eq!(gpu[0xA9], 1);
}

#[test]
fn cuda_byte_histogram_empty() {
    let source: &[u8] = &[];
    let cpu = reference_byte_histogram(source);
    let program = byte_histogram_256("source", "histogram", 0);
    let inputs = inputs_for_histogram_program(&program, source);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("empty byte histogram", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA empty byte histogram dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(256);
    assert_eq!(out, cpu.to_vec());
    assert_eq!(run_histogram_u8(source), cpu.to_vec());
}

#[test]
fn cuda_byte_histogram_u8_generated_matrix_matches_cpu() {
    for case in 0..128u32 {
        let len = match case % 7 {
            0 => 0,
            1 => 1,
            2 => 31,
            3 => 256,
            4 => 257,
            5 => 1023,
            _ => 4099,
        };
        let mut state = 0x9e37_79b9_u32 ^ case.wrapping_mul(0x85eb_ca6b);
        let mut source = Vec::with_capacity(len);
        for i in 0..len {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let byte = match (state.wrapping_add(i as u32).wrapping_add(case)) % 19 {
                0 => 0,
                1 => 0xFF,
                2 | 3 => b'\n',
                4 => b'\r',
                5 | 6 => 0xC3,
                7 => 0xA9,
                _ => (state >> 8) as u8,
            };
            source.push(byte);
        }

        assert_eq!(
            run_histogram_u8(&source),
            reference_byte_histogram(&source).to_vec(),
            "Fix: packed-u8 CUDA byte_histogram mismatch on generated case {case}"
        );
    }
}

fn run_shape_counts(histogram: &[u32; 256]) -> (u32, u32) {
    let program = utf8_shape_counts("histogram", "out");
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(histogram)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("UTF-8 shape counts", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA UTF-8 shape-count dispatch failed: {error}"))
    });
    let out = bytes_u32(&outputs[0]);
    (out[0], out[1])
}

#[test]
fn cuda_utf8_shape_counts_two_byte_seq() {
    // One 2-byte sequence: 0xC3 0xA9. Expect continuation=1, expected=1.
    let mut histogram = [0u32; 256];
    histogram[0xC3] = 1;
    histogram[0xA9] = 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (1, 1));
}

#[test]
fn cuda_utf8_shape_counts_two_two_byte_seqs() {
    // Two 2-byte sequences: continuation=2, expected=2.
    let mut histogram = [0u32; 256];
    histogram[0xC3] = 1;
    histogram[0xA9] = 1;
    histogram[0xC2] = 1;
    histogram[0x80] = 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (2, 2));
}

#[test]
fn cuda_utf8_shape_counts_three_byte_seq() {
    // 3-byte sequence (0xE0..0xEF): expected += count*2 = 2 continuations.
    let mut histogram = [0u32; 256];
    histogram[0xE2] = 1;
    histogram[0x82] = 1;
    histogram[0xAC] = 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (2, 2));
}

#[test]
fn cuda_utf8_shape_counts_four_byte_seq() {
    // 4-byte sequence (0xF0..0xF4): expected += count*3 = 3 continuations.
    let mut histogram = [0u32; 256];
    histogram[0xF0] = 1;
    histogram[0x9F] = 1;
    histogram[0x98] = 1;
    histogram[0x80] = 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (3, 3));
}

#[test]
fn cuda_utf8_shape_counts_ascii_only_zero() {
    let mut histogram = [0u32; 256];
    for b in 0u8..0x80 {
        histogram[b as usize] = 1;
    }
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (0, 0));
}

#[test]
fn cuda_utf8_shape_counts_saturates_three_byte_expected_count() {
    let mut histogram = [0u32; 256];
    histogram[0xE0] = u32::MAX / 2 + 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (0, u32::MAX));
}

#[test]
fn cuda_utf8_shape_counts_saturates_four_byte_expected_count() {
    let mut histogram = [0u32; 256];
    histogram[0xF0] = u32::MAX / 3 + 1;
    let cpu = reference_utf8_shape_counts(&histogram);
    let gpu = run_shape_counts(&histogram);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, (0, u32::MAX));
}

//! Parity test: vyre-primitives text primitives match reference oracles.
//! Covers line_index (newline-aware line counter) and utf8_validate
//! (per-byte UTF-8 class).

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::ir::{BufferAccess, DataType, Program};
use vyre::DispatchConfig;
use vyre_primitives::text::line_index::{line_index, line_index_u8, reference_line_index};
use vyre_primitives::text::utf8_validate::{
    reference_utf8_validate, utf8_validate, utf8_validate_dispatch_grid, utf8_validate_u8,
    UTF8_CONT, UTF8_INVALID, UTF8_LEAD_3, UTF8_LEAD_4,
};

fn bytes_to_u32_per_lane(source: &[u8]) -> Vec<u32> {
    source.iter().map(|&b| b as u32).collect()
}

fn inputs_for_program(program: &Program, source: &[u8]) -> Vec<Vec<u8>> {
    program
        .buffers()
        .iter()
        .filter_map(|buffer| {
            let backend_allocated = buffer.is_output() || buffer.is_pipeline_live_out();
            let needs_input = matches!(
                buffer.access(),
                BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
            ) && !backend_allocated
                && buffer.access() != BufferAccess::Workgroup;
            if !needs_input {
                return None;
            }
            if buffer.name() == "source" {
                match buffer.element() {
                    DataType::U8 => Some(source.to_vec()),
                    DataType::U32 => Some(u32_bytes(&bytes_to_u32_per_lane(source))),
                    other => {
                        panic!("Fix: CUDA text source buffer must be U8 or U32, got {other:?}")
                    }
                }
            } else {
                Some(vec![0u8; buffer.count().max(1) as usize * 4])
            }
        })
        .collect()
}

fn output_index(program: &Program, name: &str) -> usize {
    program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || buffer.is_pipeline_live_out()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|buffer| buffer.name() == name)
        .expect("Fix: CUDA text primitive output buffer must be declared")
}

fn run_line_index(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = line_index("source", "lines", n);
    let inputs = inputs_for_program(&program, source);
    let config = DispatchConfig::default();
    let lines_index = output_index(&program, "lines");
    let outputs = with_live_backend("line index primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA line-index dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[lines_index]);
    out.truncate(n as usize);
    out
}

fn run_line_index_u8(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = line_index_u8("source", "lines", n);
    let inputs = inputs_for_program(&program, source);
    let config = DispatchConfig::default();
    let lines_index = output_index(&program, "lines");
    let outputs = with_live_backend("packed-u8 line index primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA packed-u8 line-index dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[lines_index]);
    out.truncate(n as usize);
    out
}

#[test]
fn cuda_line_index_no_newlines() {
    let s = b"Hello";
    let cpu = reference_line_index(s);
    let gpu = run_line_index(s);
    let gpu_u8 = run_line_index_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![0u32; 5]);
}

#[test]
fn cuda_line_index_lf_only() {
    let s = b"ab\ncd";
    let cpu = reference_line_index(s);
    let gpu = run_line_index(s);
    let gpu_u8 = run_line_index_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![0, 0, 0, 1, 1]);
}

#[test]
fn cuda_line_index_crlf() {
    let s = b"ab\r\ncd";
    let cpu = reference_line_index(s);
    let gpu = run_line_index(s);
    let gpu_u8 = run_line_index_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![0, 0, 0, 0, 1, 1]);
}

#[test]
fn cuda_line_index_lone_cr() {
    let s = b"ab\rcd";
    let cpu = reference_line_index(s);
    let gpu = run_line_index(s);
    let gpu_u8 = run_line_index_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![0, 0, 0, 1, 1]);
}

#[test]
fn cuda_line_index_back_to_back_lf() {
    let s = b"a\n\nb";
    let cpu = reference_line_index(s);
    let gpu = run_line_index(s);
    let gpu_u8 = run_line_index_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu, vec![0, 0, 1, 2]);
}

#[test]
fn cuda_line_index_multi_block_mixed_newlines() {
    let mut s = Vec::with_capacity(4099);
    for i in 0..4099u32 {
        let byte = match i % 17 {
            0 => b'\n',
            5 => b'\r',
            6 => b'\n',
            11 => b'\r',
            _ => b'a' + (i % 23) as u8,
        };
        s.push(byte);
    }
    let cpu = reference_line_index(&s);
    let gpu = run_line_index(&s);
    let gpu_u8 = run_line_index_u8(&s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

fn generated_line_index_source(case: u32, len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_u32 ^ case.wrapping_mul(0x85eb_ca6b);
    let mut source = Vec::with_capacity(len);
    for i in 0..len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let selector = state.wrapping_add(i as u32).wrapping_add(case) % 31;
        let byte = match selector {
            0 | 7 => b'\n',
            1 | 11 => b'\r',
            2 => 0,
            3 => 0xFF,
            _ => b'a' + ((state >> 8) % 26) as u8,
        };
        source.push(byte);
    }
    if len > 260 {
        match case % 4 {
            0 => {
                source[255] = b'\r';
                source[256] = b'\n';
            }
            1 => {
                source[255] = b'\r';
                source[256] = b'x';
            }
            2 => {
                source[255] = b'\n';
                source[256] = b'\n';
            }
            _ => {
                source[254] = b'\r';
                source[255] = b'\r';
                source[256] = b'\n';
            }
        }
    }
    source
}

#[test]
fn cuda_line_index_u8_generated_matrix_matches_cpu() {
    let len = 513usize;
    let program = line_index_u8("source", "lines", len as u32);
    let lines_index = output_index(&program, "lines");
    let config = DispatchConfig::default();

    with_live_backend("packed-u8 generated line-index matrix", |backend| {
        for case in 0..128u32 {
            let source = generated_line_index_source(case, len);
            let inputs = inputs_for_program(&program, &source);
            let outputs = backend
                .dispatch(&program, &inputs, &config)
                .unwrap_or_else(|error| {
                    panic!("Fix: CUDA packed-u8 line-index generated case {case} failed: {error}")
                });
            let mut gpu = bytes_u32(&outputs[lines_index]);
            gpu.truncate(len);
            assert_eq!(
                gpu,
                reference_line_index(&source),
                "Fix: packed-u8 CUDA line_index mismatch on generated case {case}"
            );
        }
    });
}

fn run_utf8_validate(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = utf8_validate("source", "classes", n);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(&bytes_to_u32_per_lane(source))];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(utf8_validate_dispatch_grid(n));
    let outputs = with_live_backend("UTF-8 validate primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA UTF-8 validate dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(n as usize);
    out
}

fn run_utf8_validate_u8(source: &[u8]) -> Vec<u32> {
    let n = source.len() as u32;
    let program = utf8_validate_u8("source", "classes", n);
    let inputs = inputs_for_program(&program, source);
    let mut config = DispatchConfig::default();
    config.grid_override = Some(utf8_validate_dispatch_grid(n));
    let classes_index = output_index(&program, "classes");
    let outputs = with_live_backend("packed-u8 UTF-8 validate primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA packed-u8 UTF-8 validate dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[classes_index]);
    out.truncate(n as usize);
    out
}

#[test]
fn cuda_utf8_validate_ascii() {
    let s = b"Hello";
    let cpu = reference_utf8_validate(s);
    let gpu = run_utf8_validate(s);
    let gpu_u8 = run_utf8_validate_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_utf8_validate_two_byte_sequence() {
    // U+00E9 = é = 0xC3 0xA9.
    let s = &[0xC3, 0xA9];
    let cpu = reference_utf8_validate(s);
    let gpu = run_utf8_validate(s);
    let gpu_u8 = run_utf8_validate_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_utf8_validate_four_byte_sequence() {
    // U+1F600 = 😀 = 0xF0 0x9F 0x98 0x80.
    let s = &[0xF0, 0x9F, 0x98, 0x80];
    let cpu = reference_utf8_validate(s);
    let gpu = run_utf8_validate(s);
    let gpu_u8 = run_utf8_validate_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_utf8_validate_invalid_overlong_starts() {
    let s = &[0xC0u8, 0xC1];
    let cpu = reference_utf8_validate(s);
    let gpu = run_utf8_validate(s);
    let gpu_u8 = run_utf8_validate_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_utf8_validate_lone_continuation() {
    let s = &[0x80u8];
    let cpu = reference_utf8_validate(s);
    let gpu = run_utf8_validate(s);
    let gpu_u8 = run_utf8_validate_u8(s);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
}

#[test]
fn cuda_utf8_validate_mixed_input_past_first_workgroup() {
    let mut s = vec![b'x'; 777];
    s[254..258].copy_from_slice(&[0xF0, 0x9F, 0x98, 0x80]);
    s[511..514].copy_from_slice(&[0xE2, 0x82, 0xAC]);
    s[700] = 0xC0;

    let cpu = reference_utf8_validate(&s);
    let gpu = run_utf8_validate(&s);
    let gpu_u8 = run_utf8_validate_u8(&s);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu_u8, cpu);
    assert_eq!(gpu[254], UTF8_LEAD_4);
    assert_eq!(&gpu[255..258], &[UTF8_CONT, UTF8_CONT, UTF8_CONT]);
    assert_eq!(gpu[511], UTF8_LEAD_3);
    assert_eq!(&gpu[512..514], &[UTF8_CONT, UTF8_CONT]);
    assert_eq!(gpu[700], UTF8_INVALID);
}

fn generated_utf8_source(case: u32, len: usize) -> Vec<u8> {
    let mut source = Vec::with_capacity(len);
    let mut state = 0x85eb_ca6b_u32 ^ case.wrapping_mul(0x9e37_79b9);
    for index in 0..len {
        state = state
            .rotate_left(11)
            .wrapping_mul(0xc2b2_ae35)
            .wrapping_add(index as u32);
        let byte = match state & 15 {
            0 => 0xC0,
            1 => 0xC1,
            2 => 0xF5,
            3 => 0xFF,
            4 | 5 => 0x80 + ((state >> 8) % 0x40) as u8,
            6 | 7 => 0xC2 + ((state >> 11) % 0x1E) as u8,
            8 => 0xE0 + ((state >> 16) % 0x10) as u8,
            9 => 0xF0 + ((state >> 20) % 5) as u8,
            _ => (state & 0x7F) as u8,
        };
        source.push(byte);
    }

    for &offset in &[0usize, 1, 254, 255, 256, 510, 511, 768] {
        if offset + 4 <= source.len() {
            match (case + offset as u32) % 3 {
                0 => source[offset..offset + 2].copy_from_slice(&[0xC3, 0xA9]),
                1 => source[offset..offset + 3].copy_from_slice(&[0xE2, 0x82, 0xAC]),
                _ => source[offset..offset + 4].copy_from_slice(&[0xF0, 0x9F, 0x98, 0x80]),
            }
        }
    }
    source
}

#[test]
fn cuda_utf8_validate_u8_generated_matrix_matches_cpu() {
    let len = 1025usize;
    let program = utf8_validate_u8("source", "classes", len as u32);
    let classes_index = output_index(&program, "classes");
    let mut config = DispatchConfig::default();
    config.grid_override = Some(utf8_validate_dispatch_grid(len as u32));

    with_live_backend("packed-u8 generated UTF-8 matrix", |backend| {
        let mut checked = 0usize;
        for case in 0..128u32 {
            let source = generated_utf8_source(case, len);
            let inputs = inputs_for_program(&program, &source);
            let outputs = backend
                .dispatch(&program, &inputs, &config)
                .unwrap_or_else(|error| {
                    panic!("Fix: CUDA packed-u8 UTF-8 generated case {case} failed: {error}")
                });
            let mut gpu = bytes_u32(&outputs[classes_index]);
            gpu.truncate(len);
            assert_eq!(
                gpu,
                reference_utf8_validate(&source),
                "Fix: packed-u8 CUDA UTF-8 mismatch on generated case {case}"
            );
            checked += gpu.len();
        }
        assert_eq!(
            checked,
            128 * len,
            "Fix: generated packed-u8 CUDA UTF-8 matrix must compare every byte lane."
        );
    });
}

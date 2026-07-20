//! Failure-oriented adversarial tests for hash primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "hash")]

use vyre_foundation::ir::DataType;
use vyre_primitives::hash::{crc32::*, fnv1a::*};
use vyre_reference::value::Value;

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn eval_fnv1a32_u8(bytes: &[u8]) -> u32 {
    let program = fnv1a32_program_u8("input", "out", bytes.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(bytes.to_vec())])
        .expect("Fix: packed-u8 FNV-1a32 reference evaluation must succeed");
    unpack_u32s(&outputs[0].to_bytes())[0]
}

fn eval_fnv1a64_u8(bytes: &[u8]) -> u64 {
    let program = fnv1a64_program_n_u8("input", "out", bytes.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(bytes.to_vec())])
        .expect("Fix: packed-u8 FNV-1a64 reference evaluation must succeed");
    let words = unpack_u32s(&outputs[0].to_bytes());
    u64::from(words[0]) | (u64::from(words[1]) << 32)
}

#[test]
fn fnv1a32_empty_is_offset() {
    assert_eq!(fnv1a32(b""), FNV1A32_OFFSET);
}

#[test]
fn fnv1a32_all_zeros() {
    let input = vec![0u8; 1024];
    let h1 = fnv1a32(&input);
    let h2 = fnv1a32(&input);
    assert_eq!(h1, h2, "must be deterministic");
    assert_ne!(h1, FNV1A32_OFFSET, "long input should differ from empty");
}

#[test]
fn fnv1a32_all_ones() {
    let input = vec![0xFFu8; 1024];
    let h = fnv1a32(&input);
    assert_ne!(h, FNV1A32_OFFSET);
}

#[test]
fn fnv1a32_hostile_lengths() {
    for len in [0, 1, 31, 32, 33, 255, 256, 1023, 1024, 65535] {
        let input = vec![0xABu8; len];
        let hash = fnv1a32(&input);
        if len == 0 {
            assert_eq!(hash, FNV1A32_OFFSET);
        } else {
            assert_ne!(hash, FNV1A32_OFFSET);
        }
    }
}

#[test]
fn fnv1a32_single_bit_flip_changes_output() {
    let base = fnv1a32(b"hello");
    for i in 0..5usize {
        let mut mutated = *b"hello";
        mutated[i] ^= 0x01;
        let mutated_hash = fnv1a32(&mutated);
        assert_ne!(base, mutated_hash, "bit flip at byte {i} must change hash");
    }
}

#[test]
fn fnv1a64_empty_is_offset() {
    assert_eq!(fnv1a64(b""), FNV1A64_OFFSET);
}

#[test]
fn fnv1a64_hostile_lengths() {
    for len in [0, 1, 31, 32, 33, 255, 256, 1023, 1024, 65535] {
        let input = vec![0xCDu8; len];
        let hash = fnv1a64(&input);
        if len == 0 {
            assert_eq!(hash, FNV1A64_OFFSET);
        } else {
            assert_ne!(hash, FNV1A64_OFFSET);
        }
    }
}

#[test]
fn packed_u8_fnv_programs_use_one_source_byte_per_element() {
    let p32 = fnv1a32_program_u8("input", "out32", 1024);
    let p64 = fnv1a64_program_n_u8("input", "out64", 1024);

    for (program, output_name, output_words) in [(&p32, "out32", 1u32), (&p64, "out64", 2u32)] {
        let input = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "input")
            .expect("Fix: packed-u8 FNV input buffer must be declared");
        let out = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == output_name)
            .expect("Fix: packed-u8 FNV output buffer must be declared");

        assert_eq!(input.element(), DataType::U8);
        assert_eq!(input.count(), 1024);
        assert_eq!(
            input.count() as usize * DataType::U8.min_bytes(),
            1024,
            "Fix: packed-u8 FNV must consume one byte per source byte."
        );
        assert_eq!(
            input.count() as usize * DataType::U32.min_bytes(),
            4096,
            "Fix: compatibility FNV remains the four-byte-per-source-byte path."
        );
        assert_eq!(out.element(), DataType::U32);
        assert_eq!(out.count(), output_words);
    }
}

#[test]
fn packed_u8_fnv_programs_match_hostile_byte_corpus() {
    let repeated = vec![0xAB; 257];
    let cases: &[&[u8]] = &[
        b"",
        b"\x00",
        b"\xff",
        b"hello",
        b"\x00\xff\x80\x7fFNV",
        repeated.as_slice(),
    ];

    for (idx, bytes) in cases.iter().enumerate() {
        assert_eq!(
            eval_fnv1a32_u8(bytes),
            fnv1a32(bytes),
            "Fix: packed-u8 FNV-1a32 mismatch on hostile case {idx}"
        );
        assert_eq!(
            eval_fnv1a64_u8(bytes),
            fnv1a64(bytes),
            "Fix: packed-u8 FNV-1a64 mismatch on hostile case {idx}"
        );
    }
}

#[test]
fn crc32_empty_is_zero() {
    assert_eq!(crc32(b""), 0);
}

#[test]
fn crc32_hostile_lengths() {
    for len in [0, 1, 31, 32, 33, 255, 256, 1023, 1024, 65535] {
        let input = vec![0x00u8; len];
        let _ = crc32(&input);
        let input2 = vec![0xFFu8; len];
        let _ = crc32(&input2);
    }
}

#[test]
fn crc32_known_vectors() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(&[0x00]), 0xD202_EF8D);
}

#[test]
fn crc32_table_deterministic() {
    let t1 = build_table();
    let t2 = build_table();
    assert_eq!(t1, t2);
    assert_eq!(t1[0], 0);
    assert_eq!(t1[1], 0x7707_3096);
}

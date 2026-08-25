//! CRC-32 wrapper vs independent oracle matrix over hostile packed bytes.
//!
//! The oracle is implemented locally with the IEEE 802.3 reflected polynomial.
//! It is intentionally separate from `vyre_primitives::hash::crc32::crc32` so
//! this matrix catches wrapper/program regressions rather than tautologies.

#![cfg(feature = "hash")]
#![allow(deprecated)]

use vyre_reference::value::Value;

const CRC32_POLY: u32 = 0xEDB8_8320;

fn oracle_crc32_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    for index in 0..256 {
        let mut entry = index as u32;
        for _ in 0..8 {
            entry = if entry & 1 != 0 {
                (entry >> 1) ^ CRC32_POLY
            } else {
                entry >> 1
            };
        }
        table[index] = entry;
    }
    table
}

fn oracle_crc32(bytes: &[u8]) -> u32 {
    let table = oracle_crc32_table();
    let mut crc = 0xFFFF_FFFF;
    for &byte in bytes {
        let index = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ table[index];
    }
    !crc
}

fn run_crc32(words: &[u32]) -> u32 {
    let n = words.len().max(1) as u32;
    let program = vyre_libs::hash::crc32("input", "out", n);
    let input = vyre_primitives::wire::pack_u32_slice(words);
    let outputs = vyre_reference::reference_eval(&program, &[Value::Bytes(input.into())])
        .expect("Fix: crc32 reference_eval must succeed for matrix inputs.");
    let raw = outputs[0].to_bytes();
    vyre_primitives::wire::read_u32_le_word(&raw, 0, "crc32 output")
        .expect("Fix: crc32 output must contain one u32.")
}

fn hostile_packed_bytes(seed: u32) -> (Vec<u32>, Vec<u8>) {
    let len = ((seed.wrapping_mul(37) ^ seed.rotate_left(11)) % 96 + 1) as usize;
    let mut words = Vec::with_capacity(len);
    let mut bytes = Vec::with_capacity(len);
    let mut state = seed ^ 0xA5A5_5A5A;
    for index in 0..len {
        state = state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .rotate_left((index as u32) & 15);
        let byte = (state ^ (seed << (index & 7))) as u8;
        let hostile_high_bits = state & 0xFFFF_FF00;
        words.push(hostile_high_bits | u32::from(byte));
        bytes.push(byte);
    }
    (words, bytes)
}

#[test]
fn matrix_crc32_matches_independent_oracle_over_hostile_packed_inputs() {
    let mut assertions = 0usize;
    for seed in 0..512_u32 {
        let (words, bytes) = hostile_packed_bytes(seed);
        assert_eq!(
            run_crc32(&words),
            oracle_crc32(&bytes),
            "crc32 seed={seed} len={} must ignore packed-slot high bits",
            bytes.len()
        );
        assertions += 1;
    }
    assert_eq!(assertions, 512);
}

#[test]
fn matrix_crc32_canonical_vectors_match_independent_oracle() {
    let vectors: &[(&[u8], u32)] = &[(b"abc", 0x3524_41C2), (b"123456789", 0xCBF4_3926)];
    for (index, (bytes, expected)) in vectors.iter().enumerate() {
        let words: Vec<u32> = bytes.iter().map(|&byte| u32::from(byte)).collect();
        let n = words.len() as u32;
        let program = vyre_libs::hash::crc32("input", "out", n);
        let packed = vyre_primitives::wire::pack_u32_slice(&words);
        let outputs = vyre_reference::reference_eval(&program, &[Value::Bytes(packed.into())])
            .expect("Fix: canonical crc32 vector must execute.");
        let raw = outputs[0].to_bytes();
        let got = vyre_primitives::wire::read_u32_le_word(&raw, 0, "crc32 output")
            .expect("Fix: crc32 canonical output must contain one u32.");
        assert_eq!(got, *expected, "canonical vector {index}");
        assert_eq!(got, oracle_crc32(bytes), "oracle parity vector {index}");
    }
}

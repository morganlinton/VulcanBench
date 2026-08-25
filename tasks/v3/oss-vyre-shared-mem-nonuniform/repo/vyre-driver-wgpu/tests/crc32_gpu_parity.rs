//! CRC-32 (IEEE/zlib) parity on the LIVE GPU — a real shipped workload whose GPU
//! program is a NESTED loop with a per-bit conditional polynomial xor.
//!
//! `crc32_program` is tested elsewhere only at the source/oracle level, never
//! dispatched to a backend (the same oracle-only gap BLAKE3/FNV had). Its GPU
//! shape is the richest of the hash family so far: the CPU reference is
//! table-driven, but the GPU IR computes CRC bit-by-bit — an OUTER loop over the
//! input bytes and an INNER 8-iteration loop that conditionally xors the
//! reflected polynomial (0xEDB88320) based on the low bit. So this exercises a
//! nested `Node::Loop` + data-dependent conditional + shift/xor mix on real
//! silicon end-to-end. A miscompiled nested loop carrier or the conditional xor
//! would make every GPU CRC silently wrong with no test to catch it.
//!
//! Dispatched on the 5090 and asserted byte-for-byte against the `crc32` Rust
//! reference and the standard zlib CRC-32 vector for "abc".

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_primitives::hash::crc32::{crc32, crc32_program};

/// Dispatch the real `crc32_program` on the GPU: one U32 word per source byte
/// (the update masks each to its low 8 bits), single u32 CRC out at `out[0]`.
fn gpu_crc32(backend: &WgpuBackend, bytes: &[u8]) -> u32 {
    let n = bytes.len() as u32;
    let program = crc32_program("input", "out", n);
    let input_words: Vec<u32> = bytes.iter().map(|&b| u32::from(b)).collect();
    let input_b = u32_bytes(&input_words);
    let out_init = u32_bytes(&[0u32]);

    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[input_b.as_slice(), out_init.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the CRC-32 nested-loop program.");
    assert_eq!(outputs.len(), 1, "crc32_program exposes one output (out); got {}", outputs.len());
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(words.len(), 1, "out is a single u32 CRC");
    words[0]
}

fn check(backend: &WgpuBackend, bytes: &[u8], label: &str) {
    let gpu = gpu_crc32(backend, bytes);
    let expected = crc32(bytes);
    assert_eq!(
        gpu, expected,
        "GPU CRC-32 of {label} diverged from the Rust reference — the nested loop or \
         per-bit conditional polynomial xor miscompiles on hardware.\n  \
         gpu      = {gpu:#010x}\n  expected = {expected:#010x}"
    );
}

#[test]
fn crc32_abc_matches_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: CRC-32 GPU parity requires a live GPU.");
    // Drift-guard the reference against the standard zlib/IEEE CRC-32 of "abc".
    assert_eq!(crc32(b"abc"), 0x3524_41c2, "CRC-32 reference drifted for \"abc\"");
    check(&backend, b"abc", "\"abc\"");
}

#[test]
fn crc32_varied_inputs_match_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: CRC-32 GPU parity requires a live GPU.");
    check(&backend, b"a", "\"a\"");
    check(&backend, b"hello, world", "\"hello, world\"");
    check(&backend, b"\x00\xff\x80\x7f\x01", "boundary bytes");
    // 64-byte block: many outer iterations, each driving the full 8-bit inner loop.
    let long: [u8; 64] = std::array::from_fn(|i| (i as u8).wrapping_mul(31).wrapping_add(7));
    check(&backend, &long, "a 64-byte block");
    // The classic "123456789" CRC-32 check value (0xCBF43926) — a well-known KAT.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926, "CRC-32 reference drifted for the check string");
    check(&backend, b"123456789", "the CRC-32 check string");
}

#[test]
fn crc32_distinguishes_inputs_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: CRC-32 GPU parity requires a live GPU.");
    let a = gpu_crc32(&backend, b"crc-input-0");
    let b = gpu_crc32(&backend, b"crc-input-1");
    assert_ne!(a, b, "CRC-32 must distinguish one-byte-different inputs on the GPU");
    assert_eq!(a, crc32(b"crc-input-0"));
    assert_eq!(b, crc32(b"crc-input-1"));
}

//! FNV-1a 64-bit hash parity on the LIVE GPU — proves the prescribed
//! "express 64-bit as a u32 pair with explicit carry" pattern works on hardware.
//!
//! vyre deliberately rejects native 64-bit integer ARITHMETIC at the typecheck
//! boundary ("outside vyre-foundation's cross-backend arithmetic contract. Fix:
//! express the operation as a U32 pair with explicit carry"). `fnv1a64_program`
//! is the canonical example of that prescribed workaround: it carries the 64-bit
//! hash as two u32 halves (`h_lo`, `h_hi`) and synthesizes the 64-bit
//! multiply-by-FNV-prime from 16-bit partial products with an explicit carry into
//! the high word. So this is BOTH a real shipped workload (oracle-only before
//! this test, like BLAKE3/FNV-32 were) AND the empirical proof that the
//! architecture's recommended 64-bit-on-32-bit-lanes idiom is GPU-correct — if
//! the carry propagation miscompiled, every GPU FNV-64 hash would be silently
//! wrong with no test to catch it.
//!
//! Dispatched on the 5090 and asserted byte-for-byte against the `fnv1a64` Rust
//! reference (and the canonical FNV-1a 64 vector for "abc").

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_primitives::hash::fnv1a::{fnv1a64, fnv1a64_program_n};

/// Dispatch the real `fnv1a64_program_n` on the GPU. Input is one U32 word per
/// source byte; output is the 64-bit hash as two u32 words (`out[0]` = low,
/// `out[1]` = high), reconstructed `lo | (hi << 32)`.
fn gpu_fnv1a64(backend: &WgpuBackend, bytes: &[u8]) -> u64 {
    let n = bytes.len() as u32;
    let program = fnv1a64_program_n("input", "out", n);
    let input_words: Vec<u32> = bytes.iter().map(|&b| u32::from(b)).collect();
    let input_b = u32_bytes(&input_words);
    let out_init = u32_bytes(&[0u32, 0u32]);

    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[input_b.as_slice(), out_init.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the FNV-1a64 u32-pair-carry loop program.");
    assert_eq!(outputs.len(), 1, "fnv1a64_program exposes one output (out); got {}", outputs.len());
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(words.len(), 2, "out is the 64-bit hash as two u32 words");
    u64::from(words[0]) | (u64::from(words[1]) << 32)
}

fn check(backend: &WgpuBackend, bytes: &[u8], label: &str) {
    let gpu = gpu_fnv1a64(backend, bytes);
    let expected = fnv1a64(bytes);
    assert_eq!(
        gpu, expected,
        "GPU FNV-1a64 of {label} diverged from the Rust reference — the explicit \
         u32-pair carry / 64-bit multiply emulation miscompiles on hardware.\n  \
         gpu      = {gpu:#018x}\n  expected = {expected:#018x}"
    );
}

#[test]
fn fnv1a64_abc_matches_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: FNV-64 GPU parity requires a live GPU.");
    // Drift-guard the reference against the canonical FNV-1a 64 vector for "abc".
    assert_eq!(
        fnv1a64(b"abc"),
        0xe71f_a219_0541_574b,
        "FNV-1a64 reference drifted for \"abc\""
    );
    check(&backend, b"abc", "\"abc\"");
}

#[test]
fn fnv1a64_varied_inputs_match_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: FNV-64 GPU parity requires a live GPU.");
    check(&backend, b"a", "\"a\"");
    check(&backend, b"hello, world", "\"hello, world\"");
    check(&backend, b"\x00\xff\x80\x7f\x01", "boundary bytes");
    // A 64-byte block exercises the carry path across many iterations — the most
    // likely place a high-word carry bug would surface.
    let long: [u8; 64] = std::array::from_fn(|i| (i as u8).wrapping_mul(31).wrapping_add(7));
    check(&backend, &long, "a 64-byte block");
}

#[test]
fn fnv1a64_high_word_is_exercised_on_gpu() {
    // Guard that the carry into the HIGH word is actually computed (not stuck at
    // the offset's high half): a non-empty input must change the high word, and
    // two inputs differing by one byte must differ in the full 64-bit hash.
    let backend = WgpuBackend::acquire().expect("Fix: FNV-64 GPU parity requires a live GPU.");
    let a = gpu_fnv1a64(&backend, b"carry-word-0");
    let b = gpu_fnv1a64(&backend, b"carry-word-1");
    assert_ne!(a, b, "FNV-64 must distinguish one-byte-different inputs on the GPU");
    assert_ne!(
        (a >> 32) as u32,
        (fnv1a64(b"") >> 32) as u32,
        "FNV-64 high word must change for a non-empty input (carry path live)"
    );
    assert_eq!(a, fnv1a64(b"carry-word-0"));
    assert_eq!(b, fnv1a64(b"carry-word-1"));
}

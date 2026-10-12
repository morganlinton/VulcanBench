//! Adler-32 checksum parity on the LIVE GPU — a real shipped workload whose GPU
//! program carries TWO accumulators and a u32 MOD 65521 inside the loop.
//!
//! `adler32_program` was tested only at the source/oracle level, never
//! dispatched to a backend (the oracle-only gap class BLAKE3/FNV/CRC closed).
//! Its shape is distinct from the rest of the hash family: a single loop carrying
//! a DUAL state (`a` init 1, `b` init 0), each byte doing `a = (a + byte) %
//! 65521; b = (b + a) % 65521`, finalized as `(b << 16) | a`. So it exercises a
//! loop-carried PAIR plus a u32 modulo-by-a-non-power-of-2-constant on real
//! silicon — the modulo complements `div_zero_shift_mask_parity` (which only
//! covered the mod-by-zero edge) with mod-by-constant in a real workload loop.
//!
//! Dispatched on the 5090 and asserted byte-for-byte against the `adler32` Rust
//! reference and the standard zlib Adler-32 vector for "abc".

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_primitives::hash::adler32::{adler32, adler32_program};

/// Dispatch the real `adler32_program` on the GPU: one U32 word per source byte,
/// single u32 checksum `(b << 16) | a` out at `out[0]`.
fn gpu_adler32(backend: &WgpuBackend, bytes: &[u8]) -> u32 {
    let n = bytes.len() as u32;
    let program = adler32_program("input", "out", n);
    let input_words: Vec<u32> = bytes.iter().map(|&b| u32::from(b)).collect();
    let input_b = u32_bytes(&input_words);
    let out_init = u32_bytes(&[0u32]);

    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[input_b.as_slice(), out_init.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the Adler-32 dual-accumulator loop program.");
    assert_eq!(outputs.len(), 1, "adler32_program exposes one output (out); got {}", outputs.len());
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(words.len(), 1, "out is a single u32 checksum");
    words[0]
}

fn check(backend: &WgpuBackend, bytes: &[u8], label: &str) {
    let gpu = gpu_adler32(backend, bytes);
    let expected = adler32(bytes);
    assert_eq!(
        gpu, expected,
        "GPU Adler-32 of {label} diverged from the Rust reference — the dual-accumulator \
         carry or the u32 mod-65521 miscompiles on hardware.\n  \
         gpu      = {gpu:#010x}\n  expected = {expected:#010x}"
    );
}

#[test]
fn adler32_abc_matches_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: Adler-32 GPU parity requires a live GPU.");
    // Drift-guard against the standard zlib Adler-32 of "abc".
    assert_eq!(adler32(b"abc"), 0x024d_0127, "Adler-32 reference drifted for \"abc\"");
    check(&backend, b"abc", "\"abc\"");
}

#[test]
fn adler32_varied_inputs_match_reference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: Adler-32 GPU parity requires a live GPU.");
    check(&backend, b"a", "\"a\"");
    check(&backend, b"hello, world", "\"hello, world\"");
    // 0xFF bytes drive `a` and `b` up fastest — stresses the mod-65521 path.
    check(&backend, &[0xFFu8; 40], "forty 0xFF bytes");
    // 64-byte block: many iterations accumulating into both lanes before the mod.
    let long: [u8; 64] = std::array::from_fn(|i| (i as u8).wrapping_mul(31).wrapping_add(7));
    check(&backend, &long, "a 64-byte block");
    // Wikipedia's canonical Adler-32 example: "Wikipedia" -> 0x11E60398.
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398, "Adler-32 reference drifted for \"Wikipedia\"");
    check(&backend, b"Wikipedia", "\"Wikipedia\"");
}

#[test]
fn adler32_high_half_is_exercised_on_gpu() {
    // The `b` accumulator lives in the high 16 bits. A multi-byte input must move
    // it off the initial 0, and one-byte-different inputs must differ.
    let backend = WgpuBackend::acquire().expect("Fix: Adler-32 GPU parity requires a live GPU.");
    let a = gpu_adler32(&backend, b"adler-in-0");
    let b = gpu_adler32(&backend, b"adler-in-1");
    assert_ne!(a, b, "Adler-32 must distinguish one-byte-different inputs on the GPU");
    assert_ne!(a >> 16, 0, "the `b` accumulator (high half) must be non-zero for a real input");
    assert_eq!(a, adler32(b"adler-in-0"));
    assert_eq!(b, adler32(b"adler-in-1"));
}

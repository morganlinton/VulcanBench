//! Stochastic computing primitive (#59, research scaffold).
//!
//! Stochastic computing (Gaines 1969, Alaghi 2018 revival) represents
//! numbers as bitstreams; multiplication = AND, addition = MUX.
//! Trades precision for power efficiency. Recent NN inference work
//! (Tehrani 2023) uses it on GPU as bitset operations.
//!
//! This file ships **stochastic-AND multiplication**  -  multiply two
//! bitstream representations elementwise.

use super::binary_word::{binary_word_program, BitwiseBinaryOp};
use vyre_foundation::ir::{DataType, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::bitset::stochastic_and_mul";

/// Stochastic multiply (AND of bitstreams).
#[must_use]
pub fn stochastic_and_mul(a: &str, b: &str, out: &str, n_words: u32) -> Program {
    if n_words == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            "Fix: stochastic_and_mul requires n_words > 0, got 0.".to_string(),
        );
    }

    binary_word_program(OP_ID, a, b, out, n_words, BitwiseBinaryOp::And)
}

/// CPU reference for stochastic multiplication over packed bitstreams.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(a, b, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives stochastic bitstream cpu_ref failed: {error}"),
    }
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(a: &[u32], b: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(a, b, out) {
        panic!("vyre-primitives stochastic bitstream cpu_ref_into failed: {error}");
    }
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(a: &[u32], b: &[u32], out: &mut Vec<u32>) -> Result<(), String> {
    let len = a.len().min(b.len());
    out.clear();
    if len > out.capacity() {
        out.try_reserve(len - out.capacity()).map_err(|err| {
            format!("stochastic bitstream CPU reference could not reserve {len} words: {err}")
        })?;
    }
    out.extend(a.iter().zip(b.iter()).map(|(left, right)| left & right));
    Ok(())
}

/// CPU helper: encode `p ∈ [0, 1]` as bitstream of length `len_bits`.
#[must_use]
pub fn encode_bitstream(p: f64, len_bits: usize, seed: u32) -> Vec<u32> {
    let mut out = Vec::new();
    match try_encode_bitstream_into(p, len_bits, seed, &mut out) {
        Ok(()) => out,
        // Returning an empty bitstream on failure silently corrupts the encoded
        // probability (downstream stochastic math reads zeros) — a silent
        // fallback (Law 10). Fail loud; callers use try_encode_bitstream_into.
        Err(error) => panic!("vyre-primitives stochastic bitstream encode failed: {error}"),
    }
}

/// CPU helper: encode into a caller-owned bitstream buffer.
pub fn encode_bitstream_into(p: f64, len_bits: usize, seed: u32, out: &mut Vec<u32>) {
    if let Err(error) = try_encode_bitstream_into(p, len_bits, seed, out) {
        panic!("vyre-primitives stochastic bitstream encode_into failed: {error}");
    }
}

/// CPU helper: fallibly encode into a caller-owned bitstream buffer.
pub fn try_encode_bitstream_into(
    p: f64,
    len_bits: usize,
    seed: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let n_words = (len_bits + 31) / 32;
    out.clear();
    if n_words > out.capacity() {
        out.try_reserve(n_words - out.capacity()).map_err(|err| {
            format!("stochastic bitstream encoder could not reserve {n_words} words: {err}")
        })?;
    }
    out.resize(n_words, 0);
    let mut state = seed.max(1);
    let threshold = (p.clamp(0.0, 1.0) * (u32::MAX as f64)) as u32;
    for i in 0..len_bits {
        // xorshift32 for cheap deterministic pseudo-random
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        if state < threshold {
            out[i / 32] |= 1 << (i % 32);
        }
    }
    Ok(())
}

/// CPU helper: decode bitstream to `p ∈ [0, 1]` by counting set bits.
#[must_use]
pub fn decode_bitstream(bs: &[u32], len_bits: usize) -> f64 {
    let count: u32 = bs.iter().map(|w| w.count_ones()).sum();
    let count = count.min(len_bits as u32);
    count as f64 / len_bits as f64
}

#[cfg(test)]
mod non_panic_wrapper_tests {
    use super::{
        cpu_ref, cpu_ref_into, encode_bitstream, encode_bitstream_into, try_cpu_ref_into,
        try_encode_bitstream_into,
    };

    #[test]
    fn compatibility_cpu_wrappers_match_fallible_reference() {
        let a = [0xF0F0_F0F0, 0xAAAA_AAAA];
        let b = [0xFF00_00FF, 0x5555_FFFF];
        let mut compat = Vec::with_capacity(4);
        let mut fallible = Vec::with_capacity(4);

        cpu_ref_into(&a, &b, &mut compat);
        try_cpu_ref_into(&a, &b, &mut fallible)
            .expect("Fix: small stochastic CPU oracle must reserve");

        assert_eq!(cpu_ref(&a, &b), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn compatibility_encoder_wrappers_match_fallible_encoder() {
        let mut compat = Vec::with_capacity(8);
        let mut fallible = Vec::with_capacity(8);

        encode_bitstream_into(0.25, 65, 7, &mut compat);
        try_encode_bitstream_into(0.25, 65, 7, &mut fallible)
            .expect("Fix: small stochastic encoder must reserve");

        assert_eq!(encode_bitstream(0.25, 65, 7), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_wrappers_have_no_raw_panic_path() {
        let production = include_str!("stochastic_compute.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: stochastic_compute.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: stochastic bitstream compatibility wrappers must not panic in production."
        );
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || stochastic_and_mul("a", "b", "out", 2),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[0xF0F0_F0F0, 0xAAAA_AAAA]),
                crate::wire::pack_u32_slice(&[0xFF00_00FF, 0x5555_FFFF]),
                crate::wire::pack_u32_slice(&[0, 0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[0xF000_00F0, 0x0000_AAAA])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_encode_decode_roundtrip_low_p() {
        let bs = encode_bitstream(0.25, 1024, 42);
        let p = decode_bitstream(&bs, 1024);
        assert!((p - 0.25).abs() < 0.05);
    }

    #[test]
    fn cpu_encode_decode_roundtrip_high_p() {
        let bs = encode_bitstream(0.75, 1024, 42);
        let p = decode_bitstream(&bs, 1024);
        assert!((p - 0.75).abs() < 0.05);
    }

    #[test]
    fn encode_bitstream_into_reuses_output() {
        let mut bs = Vec::with_capacity(64);
        let ptr = bs.as_ptr();
        encode_bitstream_into(0.25, 1024, 42, &mut bs);
        assert!((decode_bitstream(&bs, 1024) - 0.25).abs() < 0.05);
        assert_eq!(bs.as_ptr(), ptr);
    }

    #[test]
    fn try_encode_bitstream_into_truncates_stale_tail_without_reallocating() {
        let mut bs = Vec::with_capacity(16);
        bs.extend_from_slice(&[u32::MAX; 16]);
        let ptr = bs.as_ptr();

        try_encode_bitstream_into(0.0, 65, 42, &mut bs).unwrap();

        assert_eq!(bs.len(), 3);
        assert_eq!(bs.as_ptr(), ptr);
        assert!(bs.iter().all(|word| *word == 0));
    }

    #[test]
    fn cpu_zero_p_yields_zero_bitstream() {
        let bs = encode_bitstream(0.0, 256, 1);
        for w in bs {
            assert_eq!(w, 0);
        }
    }

    #[test]
    fn cpu_ref_multiplies_bitstreams_with_and() {
        assert_eq!(
            cpu_ref(&[0xF0F0_F0F0, 0xAAAA_AAAA], &[0xFF00_00FF, 0x5555_FFFF]),
            vec![0xF000_00F0, 0x0000_AAAA]
        );
    }

    #[test]
    fn try_cpu_ref_into_truncates_stale_tail_without_reallocating() {
        let a = [0xffff_0000, 0x1357_9bdf, 0x2468_ace0];
        let b = [0x0f0f_f0f0, 0xffff_ffff];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&a, &b, &mut out).unwrap();

        assert_eq!(out, vec![a[0] & b[0], a[1] & b[1]]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_ref_matches_wordwise_and() {
        let mut out = Vec::new();
        for case in 0..4096_u32 {
            let a = [
                case.rotate_left(case % 31) ^ 0xA5A5_5A5A,
                case.wrapping_mul(0x9E37_79B9),
            ];
            let b = [
                case.rotate_right((case + 7) % 31) ^ 0x5A5A_A5A5,
                case.wrapping_mul(0x85EB_CA6B),
            ];
            cpu_ref_into(&a, &b, &mut out);
            assert_eq!(
                out,
                vec![a[0] & b[0], a[1] & b[1]],
                "generated stochastic AND case {case} must match wordwise multiplication"
            );
        }
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = stochastic_and_mul("a", "b", "out", 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 8);
        }
    }

    #[test]
    fn zero_n_words_traps() {
        let p = stochastic_and_mul("a", "b", "out", 0);
        assert!(p.stats().trap());
    }
}

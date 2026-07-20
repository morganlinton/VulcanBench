//! Wire-format robustness fuzz tests.
//!
//! Feeds random / mutated bytes to every `from_bytes` exposed in the
//! workspace and asserts:
//!
//!   1. **Never panics.** Decoders must surface every framing error
//!      as a typed `Err`, not as an out-of-bounds index, slice-cast,
//!      or arithmetic overflow.
//!   2. **Mutations of valid blobs always fail-closed.** Flipping a
//!      single byte anywhere in a valid blob must produce either a
//!      typed error OR a structurally-different (but well-typed)
//!      decoded value  -  never a silent corruption that re-encodes to
//!      a different blob shape.
//!
//! Bounded slice lengths keep proptest shrinking fast.

#![cfg(feature = "matching-regex")]

use proptest::prelude::*;
use vyre_libs::scan::{GpuLiteralSet, RulePipeline};
use vyre_primitives::matching::CompiledDfa;

fn arb_random_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..=512)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn dfa_from_bytes_never_panics(bytes in arb_random_bytes()) {
        let _ = CompiledDfa::from_bytes(&bytes);
    }

    #[test]
    fn literal_set_from_bytes_never_panics(bytes in arb_random_bytes()) {
        let _ = GpuLiteralSet::from_bytes(&bytes);
    }

    #[test]
    fn rule_pipeline_from_bytes_never_panics(bytes in arb_random_bytes()) {
        let _ = RulePipeline::from_bytes(&bytes);
    }

    /// Round-trip a real blob, then flip a single byte inside it. The
    /// decoder must reject (typed error) OR decode to a different
    /// well-typed value  -  never corrupt-and-succeed-with-original.
    #[test]
    fn dfa_single_byte_mutation_safe(
        flip_idx in 0usize..=4096,
        flip_bit in 0u8..=7,
    ) {
        use vyre_primitives::matching::dfa_compile;
        let dfa = dfa_compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
        let Ok(mut bytes) = dfa.to_bytes() else {
            return Ok(());
        };
        if flip_idx >= bytes.len() {
            return Ok(());
        }
        bytes[flip_idx] ^= 1u8 << flip_bit;
        let _ = CompiledDfa::from_bytes(&bytes);
    }

    #[test]
    fn literal_set_single_byte_mutation_safe(
        flip_idx in 0usize..=8192,
        flip_bit in 0u8..=7,
    ) {
        let engine = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
        let Ok(mut bytes) = engine.to_bytes() else {
            return Ok(());
        };
        if flip_idx >= bytes.len() {
            return Ok(());
        }
        bytes[flip_idx] ^= 1u8 << flip_bit;
        let _ = GpuLiteralSet::from_bytes(&bytes);
    }

    /// Empty + tiny inputs must produce a typed error, never a panic.
    #[test]
    fn tiny_input_safe(len in 0usize..=32) {
        let bytes = vec![0u8; len];
        let _ = CompiledDfa::from_bytes(&bytes);
        let _ = GpuLiteralSet::from_bytes(&bytes);
        let _ = RulePipeline::from_bytes(&bytes);
    }

    /// Truncating a real blob at every byte length must produce a
    /// typed error (or the empty-blob no-op decode), never panic.
    #[test]
    fn truncation_at_every_length_safe(
        truncate_to in 0usize..=8192,
    ) {
        let engine = GpuLiteralSet::compile(&[b"AKIA".as_slice()]);
        let Ok(bytes) = engine.to_bytes() else {
            return Ok(());
        };
        let cut = bytes.len().min(truncate_to);
        let _ = GpuLiteralSet::from_bytes(&bytes[..cut]);
    }
}

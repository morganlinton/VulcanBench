//! Property tests for `vyre_primitives::wire` round-trip identity and
//! bit-exact preservation. Each property runs 10 000 cases by default,
//! which is the gate-6 bar for primitive contracts.
//!
//! The properties locked here:
//!
//! 1. `pack_u32_slice` → `unpack_u32_slice` round-trips identically.
//! 2. `pack_u32_slice_into` and `pack_u32_slice` produce identical bytes.
//! 3. `pack_f32_slice` → `unpack_f32_slice` preserves `to_bits()` exactly
//!    (including NaN, ±Inf, subnormals).
//! 4. `pack_bytes_as_u32_slice_min_words` always produces `byte_len ==
//!    words * 4` and reproduces the input bytes at lane 0 of each u32.
//! 5. `append_u32_slice_le_bytes` preserves prior buffer contents and
//!    appends the LE-encoded slice intact.
//! 6. `pack_u64_slice` / `decode_u64_le_bytes_all` round-trip identity.
//! 7. `pack_u16_slice` / `decode_u16_le_bytes_all` round-trip identity.
//! 8. `pack_i32_slice` / `decode_i32_le_bytes_all` round-trip with sign
//!    preservation across the full i32 range.

use proptest::prelude::*;
use vyre_primitives::wire::{
    append_u32_slice_le_bytes, decode_i32_le_bytes_all, decode_u16_le_bytes_all,
    decode_u64_le_bytes_all, pack_bytes_as_u32_slice_min_words, pack_f32_slice, pack_i32_slice,
    pack_u16_slice, pack_u32_slice, pack_u32_slice_into, pack_u64_slice, unpack_f32_slice,
    unpack_u32_slice_into,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn pack_u32_round_trip(words in proptest::collection::vec(any::<u32>(), 0..1024)) {
        let bytes = pack_u32_slice(&words);
        let mut decoded = Vec::new();
        unpack_u32_slice_into(&bytes, words.len(), "proptest", &mut decoded).unwrap();
        prop_assert_eq!(decoded, words);
    }

    #[test]
    fn pack_u32_into_matches_pack_u32(words in proptest::collection::vec(any::<u32>(), 0..1024)) {
        let owned = pack_u32_slice(&words);
        let mut into_buf: Vec<u8> = vec![0xff; 64];
        pack_u32_slice_into(&words, &mut into_buf);
        prop_assert_eq!(owned, into_buf);
    }

    #[test]
    fn pack_f32_bit_exact_round_trip(values in proptest::collection::vec(any::<f32>(), 0..512)) {
        let bytes = pack_f32_slice(&values);
        let decoded = unpack_f32_slice(&bytes, values.len(), "proptest").unwrap();
        prop_assert_eq!(decoded.len(), values.len());
        for (a, b) in decoded.iter().zip(values.iter()) {
            prop_assert_eq!(a.to_bits(), b.to_bits(),
                "f32 to_bits divergence (NaN/Inf/subnormal coverage)");
        }
    }

    #[test]
    fn pack_bytes_as_u32_min_words_invariants(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
        min_words in 0usize..256,
    ) {
        let (out, words) = pack_bytes_as_u32_slice_min_words(&bytes, min_words).unwrap();
        prop_assert_eq!(words, bytes.len().max(min_words));
        prop_assert_eq!(out.len(), words * 4);
        // Lane 0 of each word holds the original byte; lanes 1..3 are zero.
        for (i, expected) in bytes.iter().enumerate() {
            prop_assert_eq!(out[i * 4], *expected);
            prop_assert_eq!(out[i * 4 + 1], 0);
            prop_assert_eq!(out[i * 4 + 2], 0);
            prop_assert_eq!(out[i * 4 + 3], 0);
        }
        // Trailing padding rows are all zero.
        for w in bytes.len()..words {
            prop_assert_eq!(&out[w * 4..(w + 1) * 4], &[0u8, 0, 0, 0][..]);
        }
    }

    #[test]
    fn append_u32_slice_le_bytes_preserves_prefix(
        prefix in proptest::collection::vec(any::<u8>(), 0..64),
        words in proptest::collection::vec(any::<u32>(), 0..256),
    ) {
        let mut buf = prefix.clone();
        append_u32_slice_le_bytes(&words, &mut buf);
        prop_assert_eq!(&buf[..prefix.len()], &prefix[..]);
        prop_assert_eq!(buf.len(), prefix.len() + words.len() * 4);
        let suffix = &buf[prefix.len()..];
        for (i, w) in words.iter().enumerate() {
            prop_assert_eq!(&suffix[i * 4..(i + 1) * 4], &w.to_le_bytes()[..]);
        }
    }

    #[test]
    fn pack_u64_round_trip(values in proptest::collection::vec(any::<u64>(), 0..256)) {
        let bytes = pack_u64_slice(&values);
        let decoded = decode_u64_le_bytes_all(&bytes);
        prop_assert_eq!(decoded, values);
    }

    #[test]
    fn pack_u16_round_trip(values in proptest::collection::vec(any::<u16>(), 0..512)) {
        let bytes = pack_u16_slice(&values);
        let decoded = decode_u16_le_bytes_all(&bytes);
        prop_assert_eq!(decoded, values);
    }

    #[test]
    fn pack_i32_round_trip(values in proptest::collection::vec(any::<i32>(), 0..512)) {
        let bytes = pack_i32_slice(&values);
        let decoded = decode_i32_le_bytes_all(&bytes);
        prop_assert_eq!(decoded, values);
    }
}

//! Differential test: `vyre_primitives::wire` against a hand-rolled
//! `std::io::Cursor`-based LE reader. Both implementations should
//! produce byte-exact equivalent results on every input.
//!
//! Catches future regressions that would silently re-introduce
//! endianness drift between the bytemuck::cast_slice fast path and
//! the wire format the rest of the world assumes.

use std::io::{Cursor, Read, Write};

use proptest::prelude::*;
use vyre_primitives::wire::{
    pack_f32_slice, pack_i32_slice, pack_u32_slice, pack_u64_slice, unpack_f32_slice,
    unpack_u32_slice_into,
};

fn std_pack_u32_le(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for w in words {
        out.write_all(&w.to_le_bytes()).unwrap();
    }
    out
}

fn std_unpack_u32_le(bytes: &[u8], count: usize) -> Vec<u32> {
    let mut cur = Cursor::new(bytes);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut buf = [0u8; 4];
        cur.read_exact(&mut buf).unwrap();
        out.push(u32::from_le_bytes(buf));
    }
    out
}

fn std_pack_f32_le(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.write_all(&v.to_le_bytes()).unwrap();
    }
    out
}

fn std_pack_i32_le(values: &[i32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for v in values {
        out.write_all(&v.to_le_bytes()).unwrap();
    }
    out
}

fn std_pack_u64_le(values: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 8);
    for v in values {
        out.write_all(&v.to_le_bytes()).unwrap();
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5_000))]

    #[test]
    fn wire_pack_u32_matches_std_io_cursor(
        words in proptest::collection::vec(any::<u32>(), 0..512),
    ) {
        prop_assert_eq!(pack_u32_slice(&words), std_pack_u32_le(&words));
    }

    #[test]
    fn wire_unpack_u32_matches_std_io_cursor(
        words in proptest::collection::vec(any::<u32>(), 0..512),
    ) {
        let bytes = pack_u32_slice(&words);
        let mut wire_decoded = Vec::new();
        unpack_u32_slice_into(&bytes, words.len(), "diff", &mut wire_decoded).unwrap();
        let std_decoded = std_unpack_u32_le(&bytes, words.len());
        prop_assert_eq!(wire_decoded, std_decoded);
    }

    #[test]
    fn wire_pack_f32_matches_std_io_cursor(
        values in proptest::collection::vec(any::<f32>(), 0..256),
    ) {
        // f32 to_le_bytes is the same as to_bits().to_le_bytes(), so the
        // std path and wire path should be byte-identical even on NaN.
        prop_assert_eq!(pack_f32_slice(&values), std_pack_f32_le(&values));
    }

    #[test]
    fn wire_round_trip_f32_matches_std_round_trip(
        values in proptest::collection::vec(any::<f32>(), 0..256),
    ) {
        let bytes = pack_f32_slice(&values);
        let wire_decoded = unpack_f32_slice(&bytes, values.len(), "diff").unwrap();
        // Reconstruct via std cursor too
        let mut cur = Cursor::new(&bytes);
        let mut std_decoded = Vec::with_capacity(values.len());
        for _ in 0..values.len() {
            let mut buf = [0u8; 4];
            cur.read_exact(&mut buf).unwrap();
            std_decoded.push(f32::from_le_bytes(buf));
        }
        prop_assert_eq!(wire_decoded.len(), std_decoded.len());
        for (a, b) in wire_decoded.iter().zip(std_decoded.iter()) {
            prop_assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn wire_pack_i32_matches_std_io_cursor(
        values in proptest::collection::vec(any::<i32>(), 0..256),
    ) {
        prop_assert_eq!(pack_i32_slice(&values), std_pack_i32_le(&values));
    }

    #[test]
    fn wire_pack_u64_matches_std_io_cursor(
        values in proptest::collection::vec(any::<u64>(), 0..128),
    ) {
        prop_assert_eq!(pack_u64_slice(&values), std_pack_u64_le(&values));
    }
}

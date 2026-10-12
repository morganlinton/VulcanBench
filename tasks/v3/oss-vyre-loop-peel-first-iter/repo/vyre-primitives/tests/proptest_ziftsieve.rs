//! Property and adversarial tests for primitive-owned LZ4 literal extraction.

#![cfg(feature = "decode")]

use proptest::prelude::*;
use vyre_primitives::decode::ziftsieve::ziftsieve_reference_extract_literals;

fn literal_only_lz4(bytes: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(bytes.len() + (bytes.len() / 14 + 1) * 3);
    let mut chunks = bytes.chunks(14).peekable();
    while let Some(chunk) = chunks.next() {
        encoded.push((chunk.len() as u8) << 4);
        encoded.extend_from_slice(chunk);
        if chunks.peek().is_some() {
            encoded.extend_from_slice(&[0, 0]);
        }
    }
    encoded
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn literal_only_blocks_round_trip(bytes in proptest::collection::vec(any::<u8>(), 0..768)) {
        let encoded = literal_only_lz4(&bytes);
        prop_assert_eq!(ziftsieve_reference_extract_literals(&encoded, bytes.len()).unwrap(), bytes);
    }
}

#[test]
fn reference_honors_output_cap_without_overallocating() {
    let encoded = literal_only_lz4(b"abcdefghijklmnopqrstuvwxyz");
    let got = ziftsieve_reference_extract_literals(&encoded, 7).unwrap();
    assert_eq!(got, b"abcdefg");
}

#[test]
fn reference_rejects_truncated_extended_literal_length() {
    let err = ziftsieve_reference_extract_literals(&[0xF0], 1024).unwrap_err();
    assert!(err.contains("truncated length encoding"));
    assert!(err.contains("Fix:"));
}

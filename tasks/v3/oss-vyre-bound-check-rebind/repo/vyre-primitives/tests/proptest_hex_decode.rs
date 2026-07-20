//! Property and adversarial tests for the primitive-owned hex decode oracle.
#![cfg(feature = "decode")]

use proptest::prelude::*;
use vyre_primitives::decode::hex::{
    hex_decode_reference_packed, hex_decode_table, hex_decoded_capacity,
};

fn manual_hex_decode(input: &[u8]) -> Vec<u32> {
    let table = hex_decode_table();
    input
        .chunks_exact(2)
        .map(|pair| (table[usize::from(pair[0])] << 4) | table[usize::from(pair[1])])
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn packed_reference_matches_independent_table_decode(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let usable_len = bytes.len() - (bytes.len() % 2);
        let input = &bytes[..usable_len];
        prop_assert_eq!(hex_decode_reference_packed(input), manual_hex_decode(input));
        prop_assert_eq!(hex_decode_reference_packed(input).len() as u32, hex_decoded_capacity(input.len() as u32));
    }
}

#[test]
fn adversarial_invalid_nibbles_clamp_to_zero() {
    assert_eq!(hex_decode_reference_packed(b"Zz**00"), vec![0, 0, 0]);
}

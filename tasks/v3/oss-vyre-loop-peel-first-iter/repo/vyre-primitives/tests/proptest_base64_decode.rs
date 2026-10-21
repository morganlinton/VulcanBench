//! Property and adversarial tests for the primitive-owned base64 decode oracle.
#![cfg(feature = "decode")]

use proptest::prelude::*;
use vyre_primitives::decode::base64::{
    decode_standard_packed_reference, decoded_capacity, standard_decode_table, INVALID,
};

fn manual_decode(input: &[u8]) -> (Vec<u32>, u32) {
    let table = standard_decode_table();
    let mut out = vec![0u32; (input.len() / 4) * 3];
    for block in 0..(input.len() / 4) {
        let base = block * 4;
        let mut vals = [
            table[input[base] as usize],
            table[input[base + 1] as usize],
            table[input[base + 2] as usize],
            table[input[base + 3] as usize],
        ];
        for value in &mut vals {
            if *value == INVALID {
                *value = 0;
            }
        }
        let out_base = block * 3;
        out[out_base] = (vals[0] << 2) | (vals[1] >> 4);
        if input[base + 2] != b'=' {
            out[out_base + 1] = ((vals[1] & 0x0F) << 4) | (vals[2] >> 2);
        }
        if input[base + 3] != b'=' {
            out[out_base + 2] = ((vals[2] & 0x03) << 6) | vals[3];
        }
    }
    let mut decoded_len = out.len() as u32;
    if input.len() >= 2 {
        if input[input.len() - 1] == b'=' {
            decoded_len = decoded_len.saturating_sub(1);
        }
        if input[input.len() - 2] == b'=' {
            decoded_len = decoded_len.saturating_sub(1);
        }
    }
    (out, decoded_len)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn standard_reference_matches_independent_decode(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let usable_len = bytes.len() - (bytes.len() % 4);
        let input = &bytes[..usable_len];
        prop_assert_eq!(decode_standard_packed_reference(input), manual_decode(input));
    }
}

#[test]
fn adversarial_padding_and_invalid_bytes_keep_fixed_capacity() {
    let cases: &[&[u8]] = &[
        b"====",
        b"AA==",
        b"AAA=",
        b"****",
        b"TWE=",
        b"SGVsbG8*",
        b"\0\0\0\0",
    ];
    for input in cases {
        let (decoded, decoded_len) = decode_standard_packed_reference(input);
        assert_eq!(decoded.len(), decoded_capacity(input.len() as u32) as usize);
        assert!(
            decoded_len <= decoded.len() as u32,
            "Fix: decoded length must never exceed fixed GPU output capacity for {input:?}."
        );
    }
}

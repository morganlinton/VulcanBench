//! Property gates for `vyre_primitives::hash::fnv1a::fnv1a32`.

#![cfg(feature = "hash")]

use proptest::prelude::*;
use vyre_foundation::ir::DataType;
use vyre_primitives::hash::fnv1a::{
    fnv1a32, fnv1a32_initial_state, fnv1a32_program_u8, fnv1a32_update_byte, fnv1a64,
    fnv1a64_program_n_u8,
};
use vyre_reference::value::Value;

fn manual_fnv1a32(bytes: &[u8]) -> u32 {
    let mut h = fnv1a32_initial_state();
    for &byte in bytes {
        h = fnv1a32_update_byte(h, byte);
    }
    h
}

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn run_fnv1a32_u8_program(bytes: &[u8]) -> u32 {
    let program = fnv1a32_program_u8("input", "out", bytes.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(bytes.to_vec())])
        .expect("Fix: packed-u8 FNV-1a32 reference evaluation must succeed");
    unpack_u32s(&outputs[0].to_bytes())[0]
}

fn run_fnv1a64_u8_program(bytes: &[u8]) -> u64 {
    let program = fnv1a64_program_n_u8("input", "out", bytes.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(bytes.to_vec())])
        .expect("Fix: packed-u8 FNV-1a64 reference evaluation must succeed");
    let words = unpack_u32s(&outputs[0].to_bytes());
    u64::from(words[0]) | (u64::from(words[1]) << 32)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn fnv1a32_matches_manual(bytes in proptest::collection::vec(any::<u8>(), 0..=64)) {
        prop_assert_eq!(fnv1a32(&bytes), manual_fnv1a32(&bytes));
    }

    #[test]
    fn empty_slice_is_offset_basis(_dummy in 0..1) {
        let _ = _dummy;
        prop_assert_eq!(fnv1a32(&[]), fnv1a32_initial_state());
    }

    #[test]
    fn single_byte_matches_manual(b in any::<u8>()) {
        prop_assert_eq!(fnv1a32(&[b]), manual_fnv1a32(&[b]));
    }

    #[test]
    fn concatenation_property(a in proptest::collection::vec(any::<u8>(), 0..=32), b in proptest::collection::vec(any::<u8>(), 0..=32)) {
        let mut concat = a.clone();
        concat.extend_from_slice(&b);
        let hash_concat = fnv1a32(&concat);
        let hash_a = fnv1a32(&a);
        // FNV-1a is not associative, but we can verify the incremental property:
        // hash(a || b) == continue_from(hash(a), b)
        let mut h = hash_a;
        for &byte in &b {
            h = fnv1a32_update_byte(h, byte);
        }
        prop_assert_eq!(hash_concat, h);
    }

    #[test]
    fn packed_u8_builders_keep_byte_source(n in 0u32..=4096) {
        let p32 = fnv1a32_program_u8("input", "out32", n);
        let p64 = fnv1a64_program_n_u8("input", "out64", n);

        for (program, output_words) in [(&p32, 1u32), (&p64, 2u32)] {
            let source = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name() == "input")
                .expect("Fix: packed-u8 FNV input buffer must be declared");
            let out = program
                .buffers()
                .iter()
                .find(|buffer| buffer.name().starts_with("out"))
                .expect("Fix: packed-u8 FNV output buffer must be declared");

            prop_assert_eq!(source.element(), DataType::U8);
            prop_assert_eq!(source.count(), n);
            prop_assert_eq!(out.element(), DataType::U32);
            prop_assert_eq!(out.count(), output_words);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_048))]

    #[test]
    fn packed_u8_programs_match_byte_hashers(bytes in proptest::collection::vec(any::<u8>(), 0..=256)) {
        prop_assert_eq!(run_fnv1a32_u8_program(&bytes), fnv1a32(&bytes));
        prop_assert_eq!(run_fnv1a64_u8_program(&bytes), fnv1a64(&bytes));
    }
}

//! Property gates for `vyre_primitives::text::utf8_validate::reference_utf8_validate`.
#![cfg(all(feature = "text", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_foundation::ir::DataType;
use vyre_primitives::text::utf8_validate::{
    reference_utf8_validate, utf8_validate_u8, UTF8_ASCII, UTF8_CONT, UTF8_INVALID, UTF8_LEAD_2,
    UTF8_LEAD_3, UTF8_LEAD_4,
};
use vyre_reference::value::Value;

fn weighted_utf8_byte() -> impl Strategy<Value = u8> {
    prop_oneof![
        10 => 0x00u8..=0x7f,
        4 => 0x80u8..=0xbf,
        4 => 0xc2u8..=0xdf,
        2 => 0xe0u8..=0xef,
        2 => 0xf0u8..=0xf4,
        1 => prop_oneof![Just(0xc0u8), Just(0xc1u8), Just(0xf5u8), Just(0xffu8)],
        8 => any::<u8>(),
    ]
}

fn is_cont(byte: u8) -> bool {
    (0x80..=0xbf).contains(&byte)
}

fn valid_lead3(first: u8, second: u8, third: u8) -> bool {
    let second_ok = match first {
        0xe0 => (0xa0..=0xbf).contains(&second),
        0xe1..=0xec | 0xee..=0xef => is_cont(second),
        0xed => (0x80..=0x9f).contains(&second),
        _ => false,
    };
    second_ok && is_cont(third)
}

fn valid_lead4(first: u8, second: u8, third: u8, fourth: u8) -> bool {
    let second_ok = match first {
        0xf0 => (0x90..=0xbf).contains(&second),
        0xf1..=0xf3 => is_cont(second),
        0xf4 => (0x80..=0x8f).contains(&second),
        _ => false,
    };
    second_ok && is_cont(third) && is_cont(fourth)
}

fn independent_utf8_validate(source: &[u8]) -> Vec<u32> {
    let mut out = vec![UTF8_INVALID; source.len()];
    let mut index = 0usize;
    while index < source.len() {
        let byte = source[index];
        if byte <= 0x7f {
            out[index] = UTF8_ASCII;
            index += 1;
            continue;
        }
        if (0xc2..=0xdf).contains(&byte) && source.get(index + 1).copied().is_some_and(is_cont) {
            out[index] = UTF8_LEAD_2;
            out[index + 1] = UTF8_CONT;
            index += 2;
            continue;
        }
        if index + 2 < source.len() && valid_lead3(byte, source[index + 1], source[index + 2]) {
            out[index] = UTF8_LEAD_3;
            out[index + 1] = UTF8_CONT;
            out[index + 2] = UTF8_CONT;
            index += 3;
            continue;
        }
        if index + 3 < source.len()
            && valid_lead4(
                byte,
                source[index + 1],
                source[index + 2],
                source[index + 3],
            )
        {
            out[index] = UTF8_LEAD_4;
            out[index + 1] = UTF8_CONT;
            out[index + 2] = UTF8_CONT;
            out[index + 3] = UTF8_CONT;
            index += 4;
            continue;
        }
        index += 1;
    }
    out
}

fn generated_utf8_case(case: u32) -> Vec<u8> {
    let len = 1 + (case as usize % 640);
    let mut source = Vec::with_capacity(len + 16);
    let mut state = case.wrapping_mul(0x9e37_79b9).wrapping_add(0x85eb_ca6b);
    for index in 0..len {
        state = state
            .rotate_left(13)
            .wrapping_mul(0xc2b2_ae35)
            .wrapping_add(index as u32);
        let byte = match state & 15 {
            0 => 0xc0,
            1 => 0xc1,
            2 => 0xf5,
            3 => 0xff,
            4 | 5 => 0x80 + ((state >> 8) % 0x40) as u8,
            6 | 7 => 0xc2 + ((state >> 11) % 0x1e) as u8,
            8 => 0xe0 + ((state >> 16) % 0x10) as u8,
            9 => 0xf0 + ((state >> 20) % 5) as u8,
            _ => (state & 0x7f) as u8,
        };
        source.push(byte);
    }

    for &offset in &[0usize, 1, 254, 255, 256, 510, 511] {
        if offset + 4 <= source.len() {
            match (case + offset as u32) % 3 {
                0 => source[offset..offset + 2].copy_from_slice(&[0xc3, 0xa9]),
                1 => source[offset..offset + 3].copy_from_slice(&[0xe2, 0x82, 0xac]),
                _ => source[offset..offset + 4].copy_from_slice(&[0xf0, 0x9f, 0x98, 0x80]),
            }
        }
    }
    source
}

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn run_packed_u8_program(source: &[u8]) -> Vec<u32> {
    let program = utf8_validate_u8("source", "classes", source.len() as u32);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(source.to_vec())])
        .expect("Fix: packed-u8 UTF-8 validator reference evaluation must succeed");
    let mut out = unpack_u32s(&outputs[0].to_bytes());
    out.truncate(source.len());
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn empty_source_is_empty(_dummy in 0..1) {
        let _ = _dummy;
        prop_assert_eq!(reference_utf8_validate(&[]), Vec::<u32>::new());
    }

    #[test]
    fn all_ascii_are_class_zero(len in 0usize..64) {
        let bytes = vec![0x41u8; len];
        let expected = vec![UTF8_ASCII; len];
        prop_assert_eq!(reference_utf8_validate(&bytes), expected);
    }

    #[test]
    fn valid_two_byte_sequence(_dummy in 0..1) {
        let _ = _dummy;
        // U+00E9 (é) = 0xC3 0xA9
        prop_assert_eq!(reference_utf8_validate(&[0xC3, 0xA9]), vec![UTF8_LEAD_2, UTF8_CONT]);
    }

    #[test]
    fn valid_three_byte_sequence(_dummy in 0..1) {
        let _ = _dummy;
        // U+20AC (€) = 0xE2 0x82 0xAC
        prop_assert_eq!(reference_utf8_validate(&[0xE2, 0x82, 0xAC]), vec![UTF8_LEAD_3, UTF8_CONT, UTF8_CONT]);
    }

    #[test]
    fn standalone_continuation_byte_is_invalid(b in 0x80u8..=0xBF) {
        prop_assert_eq!(reference_utf8_validate(&[b]), vec![UTF8_INVALID]);
    }

    #[test]
    fn invalid_lead_bytes_are_invalid(b in proptest::prop_oneof![Just(0xC0), Just(0xC1), Just(0xF5), Just(0xFF)]) {
        prop_assert_eq!(reference_utf8_validate(&[b]), vec![UTF8_INVALID]);
    }

    #[test]
    fn deterministic_over_repeated_calls(bytes in proptest::collection::vec(any::<u8>(), 0..=32)) {
        let a = reference_utf8_validate(&bytes);
        let b = reference_utf8_validate(&bytes);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn generated_bytes_match_independent_oracle(
        bytes in proptest::collection::vec(weighted_utf8_byte(), 0..=512),
    ) {
        prop_assert_eq!(
            reference_utf8_validate(&bytes),
            independent_utf8_validate(&bytes),
            "len={}",
            bytes.len()
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_048))]

    #[test]
    fn packed_u8_program_matches_independent_oracle(
        bytes in proptest::collection::vec(weighted_utf8_byte(), 0..=256),
    ) {
        prop_assert_eq!(
            run_packed_u8_program(&bytes),
            independent_utf8_validate(&bytes),
            "len={}",
            bytes.len()
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn packed_u8_builder_declares_byte_source(
        n in 0u32..=2048,
    ) {
        let program = utf8_validate_u8("source", "classes", n);
        let source = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: packed-u8 UTF-8 source buffer must be declared");
        let classes = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "classes")
            .expect("Fix: packed-u8 UTF-8 class output must be declared");

        prop_assert_eq!(program.workgroup_size(), [256, 1, 1]);
        prop_assert_eq!(source.element(), DataType::U8);
        prop_assert_eq!(source.count(), n);
        prop_assert_eq!(classes.element(), DataType::U32);
        prop_assert!(classes.is_output());
        prop_assert_eq!(n as usize * DataType::U8.min_bytes(), n as usize);
        prop_assert_eq!(n as usize * DataType::U32.min_bytes(), n as usize * 4);
    }
}

#[test]
fn generated_boundary_matrix_matches_independent_oracle() {
    let mut saw_lead2 = false;
    let mut saw_lead3 = false;
    let mut saw_lead4 = false;
    let mut saw_invalid = false;

    for case in 0..4096u32 {
        let source = generated_utf8_case(case);
        let actual = reference_utf8_validate(&source);
        let expected = independent_utf8_validate(&source);
        assert_eq!(actual, expected, "generated UTF-8 case {case}");
        saw_lead2 |= actual.contains(&UTF8_LEAD_2);
        saw_lead3 |= actual.contains(&UTF8_LEAD_3);
        saw_lead4 |= actual.contains(&UTF8_LEAD_4);
        saw_invalid |= actual.contains(&UTF8_INVALID);
    }

    assert!(saw_lead2);
    assert!(saw_lead3);
    assert!(saw_lead4);
    assert!(saw_invalid);
}

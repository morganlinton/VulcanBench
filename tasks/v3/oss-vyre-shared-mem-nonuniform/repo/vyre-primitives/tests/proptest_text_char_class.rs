//! Property gates for `vyre_primitives::text::char_class::reference_char_class`.

#![cfg(all(feature = "text", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_foundation::ir::DataType;
use vyre_primitives::text::char_class::{
    build_char_class_table, char_class_u8, reference_char_class,
};
use vyre_reference::value::Value;

fn pack_u32s(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn unpack_u32s(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            u32::from_le_bytes(chunk.try_into().expect("Fix: u32 chunk conversion failed"))
        })
        .collect()
}

fn run_packed_u8_program(source: &[u8], table: &[u32; 256]) -> Vec<u32> {
    let program = char_class_u8("source", "classified", source.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(source.to_vec()), Value::from(pack_u32s(table))],
    )
    .expect("Fix: packed-u8 char_class reference evaluation must succeed");
    let mut classified = unpack_u32s(&outputs[0].to_bytes());
    classified.truncate(source.len());
    classified
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn every_ascii_byte_maps_via_table(
        bytes in proptest::collection::vec(any::<u8>(), 0..=256),
    ) {
        let table = build_char_class_table();
        let result = reference_char_class(&bytes, &table);
        let expected: Vec<u32> = bytes.iter().map(|b| table[usize::from(*b)]).collect();
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn table_is_deterministic(_dummy in 0u32..1) {
        let t1 = build_char_class_table();
        let t2 = build_char_class_table();
        prop_assert_eq!(t1, t2);
    }

    #[test]
    fn packed_u8_builder_keeps_byte_source(
        n in 0u32..=4096,
    ) {
        let program = char_class_u8("source", "classified", n);
        let has_u8_source = program.buffers().iter().any(|buffer| {
            buffer.name() == "source"
                && buffer.element() == DataType::U8
                && buffer.count() == n
        });
        let has_u32_table = program.buffers().iter().any(|buffer| {
            buffer.name() == "table"
                && buffer.element() == DataType::U32
                && buffer.count() == 256
        });
        let has_u32_classified = program.buffers().iter().any(|buffer| {
            buffer.name() == "classified"
                && buffer.element() == DataType::U32
                && buffer.count() == n.max(1)
                && buffer.output_byte_range()
                    == Some(0..usize::try_from(n).unwrap_or(usize::MAX).saturating_mul(4))
                && buffer.is_output()
        });

        prop_assert!(has_u8_source, "char_class_u8 source must be packed U8 for n={n}");
        prop_assert!(has_u32_table, "char_class_u8 table must remain a 256-entry U32 lookup table for n={n}");
        prop_assert!(has_u32_classified, "char_class_u8 output must remain one U32 class per source byte for n={n}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_048))]

    #[test]
    fn packed_u8_program_matches_table_reference(
        source in proptest::collection::vec(any::<u8>(), 0..=256),
        table_values in proptest::collection::vec(any::<u32>(), 256..=256),
    ) {
        let mut table = [0u32; 256];
        table.copy_from_slice(&table_values);
        prop_assert_eq!(
            run_packed_u8_program(&source, &table),
            reference_char_class(&source, &table)
        );
    }
}

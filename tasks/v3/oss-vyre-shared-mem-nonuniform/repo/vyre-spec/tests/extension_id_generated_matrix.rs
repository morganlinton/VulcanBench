//! Generated ABI matrix for extension-id families.
//!
//! Extension ids are open-world wire identifiers. These tests pin the
//! documented FNV-1a derivation, high-bit reservation rule, raw-id constructor
//! behavior, and serde scalar shape across every extension family.

use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionRuleConditionId,
    ExtensionTernaryOpId, ExtensionUnOpId,
};

const GENERATED_CASES: usize = 8192;
const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;
const EXTENSION_MASK: u32 = 0x8000_0000;

#[test]
fn generated_extension_names_match_documented_fnv1a_for_every_family() {
    for index in 0..GENERATED_CASES {
        let name = generated_extension_name(index as u64);
        let expected = fnv1a_with_high_bit(name.as_bytes());

        assert_eq!(ExtensionDataTypeId::from_name(&name).as_u32(), expected);
        assert_eq!(ExtensionBinOpId::from_name(&name).as_u32(), expected);
        assert_eq!(ExtensionUnOpId::from_name(&name).as_u32(), expected);
        assert_eq!(ExtensionAtomicOpId::from_name(&name).as_u32(), expected);
        assert_eq!(ExtensionTernaryOpId::from_name(&name).as_u32(), expected);
        assert_eq!(
            ExtensionRuleConditionId::from_name(&name).as_u32(),
            expected
        );

        assert!(ExtensionDataTypeId::from_name(&name).is_extension());
        assert!(ExtensionBinOpId::from_name(&name).is_extension());
        assert!(ExtensionUnOpId::from_name(&name).is_extension());
        assert!(ExtensionAtomicOpId::from_name(&name).is_extension());
        assert!(ExtensionTernaryOpId::from_name(&name).is_extension());
        assert!(ExtensionRuleConditionId::from_name(&name).is_extension());
    }
}

#[test]
fn generated_raw_extension_ids_preserve_range_semantics_and_serde_shape() {
    for index in 0..GENERATED_CASES {
        let raw = next_state(index as u64) as u32;
        let expected_extension = (raw & EXTENSION_MASK) != 0;

        let data_type = ExtensionDataTypeId(raw);
        let binop = ExtensionBinOpId(raw);
        let unop = ExtensionUnOpId(raw);
        let atomic = ExtensionAtomicOpId(raw);
        let ternary = ExtensionTernaryOpId(raw);
        let rule = ExtensionRuleConditionId(raw);

        assert_eq!(data_type.as_u32(), raw);
        assert_eq!(binop.as_u32(), raw);
        assert_eq!(unop.as_u32(), raw);
        assert_eq!(atomic.as_u32(), raw);
        assert_eq!(ternary.as_u32(), raw);
        assert_eq!(rule.as_u32(), raw);

        assert_eq!(data_type.is_extension(), expected_extension);
        assert_eq!(binop.is_extension(), expected_extension);
        assert_eq!(unop.is_extension(), expected_extension);
        assert_eq!(atomic.is_extension(), expected_extension);
        assert_eq!(ternary.is_extension(), expected_extension);
        assert_eq!(rule.is_extension(), expected_extension);

        assert_json_scalar_round_trip(raw, data_type);
        assert_json_scalar_round_trip(raw, binop);
        assert_json_scalar_round_trip(raw, unop);
        assert_json_scalar_round_trip(raw, atomic);
        assert_json_scalar_round_trip(raw, ternary);
        assert_json_scalar_round_trip(raw, rule);
    }
}

#[test]
fn named_extension_id_vectors_are_frozen() {
    let vectors = [
        ("", 0x811c_9dc5),
        ("dialect.tensor", 0xda74_30d0),
        ("dialect.binop", 0x9691_21ad),
        ("graph.reachability.wave", 0x9d2f_cd8b),
        ("runtime.megakernel.queue", 0xdd19_e2f9),
        ("cuda.resident.crc32.map_reduce", 0xe9b5_2b18),
    ];

    for (name, expected) in vectors {
        assert_eq!(fnv1a_with_high_bit(name.as_bytes()), expected);
        assert_eq!(ExtensionDataTypeId::from_name(name).as_u32(), expected);
        assert_eq!(ExtensionBinOpId::from_name(name).as_u32(), expected);
        assert_eq!(ExtensionUnOpId::from_name(name).as_u32(), expected);
        assert_eq!(ExtensionAtomicOpId::from_name(name).as_u32(), expected);
        assert_eq!(ExtensionTernaryOpId::from_name(name).as_u32(), expected);
        assert_eq!(ExtensionRuleConditionId::from_name(name).as_u32(), expected);
    }
}

fn assert_json_scalar_round_trip<T>(raw: u32, value: T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
{
    let encoded = serde_json::to_string(&value).expect("extension id must serialize");
    assert_eq!(encoded, raw.to_string());
    let decoded: T = serde_json::from_str(&encoded).expect("extension id must deserialize");
    assert_eq!(decoded, value);
}

fn generated_extension_name(index: u64) -> String {
    let mut state = next_state(index ^ 0xfeed_f00d_dead_beef);
    let domain = match state % 8 {
        0 => "tensor",
        1 => "graph",
        2 => "parser",
        3 => "quant",
        4 => "collective",
        5 => "autodiff",
        6 => "runtime",
        _ => "cuda",
    };
    state = next_state(state);
    let feature = match state % 8 {
        0 => "gather",
        1 => "scatter",
        2 => "fixpoint",
        3 => "dtype",
        4 => "reduce",
        5 => "gradient",
        6 => "resident",
        _ => "map_reduce",
    };
    state = next_state(state);
    format!(
        "generated.{domain}.{feature}.case_{index:04x}.{:016x}",
        next_state(state)
    )
}

fn fnv1a_with_high_bit(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash | EXTENSION_MASK
}

fn next_state(value: u64) -> u64 {
    value
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(0xd1b5_4a32_d192_ed03)
        .rotate_left(17)
}

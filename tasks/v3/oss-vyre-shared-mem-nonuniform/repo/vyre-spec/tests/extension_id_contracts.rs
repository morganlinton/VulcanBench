//! External tests for extension id wire-range contracts.
//!
//! Extension ids are the open-world escape hatch for future dialects. These
//! tests pin the shared invariants every id family must satisfy: deterministic
//! name hashing, high-bit reservation, non-degenerate low bits, and raw-id
//! preservation through serde.

use std::collections::BTreeSet;

use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionRuleConditionId,
    ExtensionTernaryOpId, ExtensionUnOpId,
};

#[test]
fn extension_id_families_are_deterministic_for_stable_names() {
    assert_eq!(
        ExtensionDataTypeId::from_name("dialect.tensor"),
        ExtensionDataTypeId::from_name("dialect.tensor")
    );
    assert_eq!(
        ExtensionBinOpId::from_name("dialect.binop"),
        ExtensionBinOpId::from_name("dialect.binop")
    );
    assert_eq!(
        ExtensionUnOpId::from_name("dialect.unop"),
        ExtensionUnOpId::from_name("dialect.unop")
    );
    assert_eq!(
        ExtensionAtomicOpId::from_name("dialect.atomic"),
        ExtensionAtomicOpId::from_name("dialect.atomic")
    );
    assert_eq!(
        ExtensionTernaryOpId::from_name("dialect.ternary"),
        ExtensionTernaryOpId::from_name("dialect.ternary")
    );
    assert_eq!(
        ExtensionRuleConditionId::from_name("dialect.rule"),
        ExtensionRuleConditionId::from_name("dialect.rule")
    );
}

#[test]
fn every_extension_id_family_exposes_high_bit_range_check() {
    assert!(ExtensionDataTypeId::from_name("dialect.tensor").is_extension());
    assert!(ExtensionBinOpId::from_name("dialect.binop").is_extension());
    assert!(ExtensionUnOpId::from_name("dialect.unop").is_extension());
    assert!(ExtensionAtomicOpId::from_name("dialect.atomic").is_extension());
    assert!(ExtensionTernaryOpId::from_name("dialect.ternary").is_extension());
    assert!(ExtensionRuleConditionId::from_name("dialect.rule").is_extension());
}

#[test]
fn sampled_extension_names_do_not_collapse_to_a_single_id() {
    let names = [
        "math.tensor.gather",
        "math.tensor.scatter",
        "graph.reachability.wave",
        "parser.c11.token",
        "security.taint.flow",
        "quant.int4.dot",
        "collective.all_reduce",
        "autodiff.reverse.gradient",
        "runtime.megakernel.queue",
        "io.gpudirect.slice",
        "formal.smt.rewrite",
        "profiling.trace.event",
    ];

    let mut data_type_ids = BTreeSet::new();
    let mut binop_ids = BTreeSet::new();
    let mut rule_ids = BTreeSet::new();
    for name in &names {
        data_type_ids.insert(ExtensionDataTypeId::from_name(name).as_u32());
        binop_ids.insert(ExtensionBinOpId::from_name(name).as_u32());
        rule_ids.insert(ExtensionRuleConditionId::from_name(name).as_u32());
    }

    assert_eq!(data_type_ids.len(), names.len());
    assert_eq!(binop_ids.len(), names.len());
    assert_eq!(rule_ids.len(), names.len());
}

#[test]
fn serde_round_trip_preserves_raw_extension_ids() {
    let data_type = ExtensionDataTypeId::from_name("serde.dtype");
    assert_ne!(data_type, ExtensionDataTypeId::from_name("serde.binop"));
    let binop = ExtensionBinOpId::from_name("serde.binop");
    let unop = ExtensionUnOpId::from_name("serde.unop");
    let atomic = ExtensionAtomicOpId::from_name("serde.atomic");
    let ternary = ExtensionTernaryOpId::from_name("serde.ternary");
    let rule = ExtensionRuleConditionId::from_name("serde.rule");

    assert_eq!(
        serde_json::from_str::<ExtensionDataTypeId>(
            &serde_json::to_string(&data_type).expect("extension data type id must serialize")
        )
        .expect("extension data type id must deserialize"),
        data_type
    );
    assert_eq!(
        serde_json::from_str::<ExtensionBinOpId>(
            &serde_json::to_string(&binop).expect("extension binop id must serialize")
        )
        .expect("extension binop id must deserialize"),
        binop
    );
    assert_eq!(
        serde_json::from_str::<ExtensionUnOpId>(
            &serde_json::to_string(&unop).expect("extension unop id must serialize")
        )
        .expect("extension unop id must deserialize"),
        unop
    );
    assert_eq!(
        serde_json::from_str::<ExtensionAtomicOpId>(
            &serde_json::to_string(&atomic).expect("extension atomic id must serialize")
        )
        .expect("extension atomic id must deserialize"),
        atomic
    );
    assert_eq!(
        serde_json::from_str::<ExtensionTernaryOpId>(
            &serde_json::to_string(&ternary).expect("extension ternary id must serialize")
        )
        .expect("extension ternary id must deserialize"),
        ternary
    );
    assert_eq!(
        serde_json::from_str::<ExtensionRuleConditionId>(
            &serde_json::to_string(&rule).expect("extension rule id must serialize")
        )
        .expect("extension rule id must deserialize"),
        rule
    );
}

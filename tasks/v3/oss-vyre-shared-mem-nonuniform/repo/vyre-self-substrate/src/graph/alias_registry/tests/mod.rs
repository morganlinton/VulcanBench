use super::*;
use std::sync::atomic::Ordering;
use vyre_foundation::ir::DataType;
use vyre_primitives::graph::alias_registry::ALIAS_UNION_OP_ID;

const GENERATED_EXTENSION_IDS: &[&str] = &[
    "vyre-primitives::graph::alias_ext.generated.000",
    "vyre-primitives::graph::alias_ext.generated.001",
    "vyre-primitives::graph::alias_ext.generated.002",
    "vyre-primitives::graph::alias_ext.generated.003",
    "vyre-primitives::graph::alias_ext.generated.004",
    "vyre-primitives::graph::alias_ext.generated.005",
    "vyre-primitives::graph::alias_ext.generated.006",
    "vyre-primitives::graph::alias_ext.generated.007",
    "vyre-primitives::graph::alias_ext.generated.008",
    "vyre-primitives::graph::alias_ext.generated.009",
    "vyre-primitives::graph::alias_ext.generated.010",
    "vyre-primitives::graph::alias_ext.generated.011",
    "vyre-primitives::graph::alias_ext.generated.012",
    "vyre-primitives::graph::alias_ext.generated.013",
    "vyre-primitives::graph::alias_ext.generated.014",
    "vyre-primitives::graph::alias_ext.generated.015",
    "vyre-primitives::graph::alias_ext.generated.016",
    "vyre-primitives::graph::alias_ext.generated.017",
    "vyre-primitives::graph::alias_ext.generated.018",
    "vyre-primitives::graph::alias_ext.generated.019",
    "vyre-primitives::graph::alias_ext.generated.020",
    "vyre-primitives::graph::alias_ext.generated.021",
    "vyre-primitives::graph::alias_ext.generated.022",
    "vyre-primitives::graph::alias_ext.generated.023",
    "vyre-primitives::graph::alias_ext.generated.024",
    "vyre-primitives::graph::alias_ext.generated.025",
    "vyre-primitives::graph::alias_ext.generated.026",
    "vyre-primitives::graph::alias_ext.generated.027",
    "vyre-primitives::graph::alias_ext.generated.028",
    "vyre-primitives::graph::alias_ext.generated.029",
    "vyre-primitives::graph::alias_ext.generated.030",
    "vyre-primitives::graph::alias_ext.generated.031",
];

#[test]
fn default_registry_has_alias_union() {
    let registry = build_default_registry();
    assert!(alias_union_registered(&registry));
}

#[test]
fn empty_registry_has_no_ops() {
    let registry = AliasRegistry::default();
    assert!(registry.is_empty());
    assert!(!alias_union_registered(&registry));
}

#[test]
fn lookup_unknown_op_returns_none() {
    let registry = build_default_registry();
    assert!(lookup_alias_op(&registry, "vyre.graph.does_not_exist").is_none());
}

/// Closure-bar: substrate path produces the same registry as
/// calling the primitive register function directly.
#[test]
fn matches_primitive_directly() {
    let via_substrate = build_default_registry();
    let via_primitive = primitive_default_alias_registry();
    assert_eq!(via_substrate.len(), via_primitive.len());
    assert_eq!(
        via_substrate.registered_op_ids(),
        via_primitive.registered_op_ids()
    );
    assert!(via_substrate.get(ALIAS_UNION_OP_ID).is_some());
    assert!(via_primitive.get(ALIAS_UNION_OP_ID).is_some());
}

/// Adversarial: looking up the alias-union op id on an empty
/// registry must return None - no implicit defaults.
#[test]
fn empty_registry_does_not_self_populate() {
    let registry = AliasRegistry::default();
    assert!(lookup_alias_op(&registry, ALIAS_UNION_OP_ID).is_none());
}

/// The default alias-union op is commutative + side-effecting
/// (the CSE / dispatch optimizer reads these flags on every
/// query). If the descriptor flips, downstream passes may
/// silently drop union calls - test pins the contract.
#[test]
fn alias_union_descriptor_contract() {
    let registry = build_default_registry();
    let desc = lookup_alias_op(&registry, ALIAS_UNION_OP_ID).unwrap();
    assert_eq!(desc.inputs, [DataType::U32, DataType::U32]);
    assert_eq!(desc.output, DataType::U32);
    assert!(desc.commutative, "alias-union must be commutative");
    assert!(desc.side_effects, "alias-union must declare side effects");
}

#[test]
fn generated_extension_updates_never_duplicate_registry_slots() {
    let mut registry = build_default_registry();
    for round in 0..128 {
        let op_id = GENERATED_EXTENSION_IDS[round % GENERATED_EXTENSION_IDS.len()];
        let descriptor = AliasOpDescriptor {
            inputs: vec![DataType::U32, DataType::U32],
            output: DataType::U32,
            description: if round % 2 == 0 {
                "generated even alias extension"
            } else {
                "generated odd alias extension"
            },
            commutative: round % 3 != 0,
            side_effects: round % 5 != 0,
        };
        registry.register(op_id, descriptor);

        let expected_len = 1 + GENERATED_EXTENSION_IDS
            .iter()
            .take(round.min(GENERATED_EXTENSION_IDS.len() - 1) + 1)
            .count();
        assert_eq!(
            registry.len(),
            expected_len,
            "Fix: generated alias extension updates must replace existing descriptor slots instead of duplicating op ids at round {round}."
        );
        assert!(
            alias_union_registered(&registry),
            "Fix: extension updates must not evict the primitive alias-union descriptor."
        );
        assert!(
            lookup_alias_op(&registry, op_id).is_some(),
            "Fix: just-registered generated alias extension must be queryable."
        );
    }
}

#[test]
fn generated_unknown_ids_never_match_alias_union_or_extensions() {
    let mut registry = build_default_registry();
    registry.register(
        GENERATED_EXTENSION_IDS[0],
        AliasOpDescriptor {
            inputs: vec![DataType::U32, DataType::U32],
            output: DataType::U32,
            description: "generated alias extension",
            commutative: true,
            side_effects: true,
        },
    );

    for unknown_id in [
        "",
        "vyre.graph.alias_union\0",
        "vyre.graph.alias_union ",
        " vyre.graph.alias_union",
        "vyre-primitives::graph::alias_ext.generated.000\0",
        "vyre-primitives::graph::alias_ext.generated.000 ",
        "vyre-primitives::graph::alias_ext.generated.032",
    ] {
        assert!(
            lookup_alias_op(&registry, unknown_id).is_none(),
            "Fix: alias registry lookups must be exact and must not normalize or prefix-match hostile op ids."
        );
    }
}

#[test]
fn alias_registry_uses_dedicated_observability_counter() {
    let alias_before = crate::observability::alias_registry_calls.load(Ordering::Relaxed);
    let dataflow_before = crate::observability::dataflow_fixpoint_calls.load(Ordering::Relaxed);

    let registry = build_default_registry();
    assert!(alias_union_registered(&registry));
    assert!(lookup_alias_op(&registry, ALIAS_UNION_OP_ID).is_some());

    let alias_after = crate::observability::alias_registry_calls.load(Ordering::Relaxed);
    let dataflow_after = crate::observability::dataflow_fixpoint_calls.load(Ordering::Relaxed);
    assert!(
        alias_after >= alias_before + 3,
        "Fix: alias-registry wrapper calls must charge the graph alias counter."
    );
    assert_eq!(
        dataflow_after, dataflow_before,
        "Fix: alias-registry wrappers must not pollute dataflow-fixpoint observability."
    );
}

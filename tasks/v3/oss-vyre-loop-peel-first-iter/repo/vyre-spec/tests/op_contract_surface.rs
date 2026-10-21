//! Surface tests for operation-contract metadata.
//!
//! Freeze-tests for `CapabilityId`, `OperationContract`, and the
//! classification enums that drive backend capability negotiation.

use vyre_spec::op_contract::{
    CapabilityId, CostHint, DeterminismClass, OperationContract, SideEffectClass,
};

// ------------------------------------------------------------------
// CapabilityId
// ------------------------------------------------------------------

#[test]
fn capability_id_new_roundtrips() {
    let cap = CapabilityId::new(" subgroup-shuffle");
    assert_eq!(cap.as_str(), " subgroup-shuffle");
}

#[test]
fn capability_id_empty_is_allowed() {
    let cap = CapabilityId::new("");
    assert_eq!(cap.as_str(), "");
}

#[test]
fn capability_id_unicode_is_allowed() {
    let cap = CapabilityId::new("缓存");
    assert_eq!(cap.as_str(), "缓存");
}

// ------------------------------------------------------------------
// OperationContract defaults
// ------------------------------------------------------------------

#[test]
fn operation_contract_none_has_all_nones() {
    let contract = OperationContract::none();
    assert!(contract.capability_requirements.is_none());
    assert!(contract.determinism.is_none());
    assert!(contract.side_effect.is_none());
    assert!(contract.cost_hint.is_none());
}

#[test]
fn operation_contract_default_is_none() {
    let contract: OperationContract = Default::default();
    assert!(contract.capability_requirements.is_none());
    assert!(contract.determinism.is_none());
    assert!(contract.side_effect.is_none());
    assert!(contract.cost_hint.is_none());
}

// ------------------------------------------------------------------
// DeterminismClass
// ------------------------------------------------------------------

#[test]
fn determinism_class_variants_are_distinct() {
    let classes = vec![
        DeterminismClass::Deterministic,
        DeterminismClass::DeterministicModuloRounding,
        DeterminismClass::NonDeterministic,
    ];
    for i in 0..classes.len() {
        for j in 0..classes.len() {
            if i == j {
                assert_eq!(classes[i], classes[j]);
            } else {
                assert_ne!(classes[i], classes[j]);
            }
        }
    }
}

// ------------------------------------------------------------------
// SideEffectClass
// ------------------------------------------------------------------

#[test]
fn side_effect_class_variants_are_distinct() {
    let classes = vec![
        SideEffectClass::Pure,
        SideEffectClass::ReadsMemory,
        SideEffectClass::WritesMemory,
        SideEffectClass::Synchronizing,
        SideEffectClass::Atomic,
    ];
    for i in 0..classes.len() {
        for j in 0..classes.len() {
            if i == j {
                assert_eq!(classes[i], classes[j]);
            } else {
                assert_ne!(classes[i], classes[j]);
            }
        }
    }
}

// ------------------------------------------------------------------
// CostHint
// ------------------------------------------------------------------

#[test]
fn cost_hint_variants_are_distinct() {
    let hints = vec![
        CostHint::Cheap,
        CostHint::Medium,
        CostHint::Expensive,
        CostHint::Unknown,
    ];
    for i in 0..hints.len() {
        for j in 0..hints.len() {
            if i == j {
                assert_eq!(hints[i], hints[j]);
            } else {
                assert_ne!(hints[i], hints[j]);
            }
        }
    }
}

// ------------------------------------------------------------------
// Serde round-trip (if enabled)
// ------------------------------------------------------------------

#[test]
fn operation_contract_serializes_to_json() {
    let contract = OperationContract {
        capability_requirements: None,
        determinism: Some(DeterminismClass::Deterministic),
        side_effect: Some(SideEffectClass::Pure),
        cost_hint: Some(CostHint::Cheap),
    };
    let json = serde_json::to_string(&contract).expect("must serialize");
    assert!(json.contains("Deterministic"));
    assert!(json.contains("Pure"));
    assert!(json.contains("Cheap"));
}

#[test]
fn operation_contract_deserializes_from_json() {
    let json = r#"{"determinism":"Deterministic","side_effect":"Pure","cost_hint":"Cheap"}"#;
    let contract: OperationContract = serde_json::from_str(json).expect("must deserialize");
    assert_eq!(contract.determinism, Some(DeterminismClass::Deterministic));
    assert_eq!(contract.side_effect, Some(SideEffectClass::Pure));
    assert_eq!(contract.cost_hint, Some(CostHint::Cheap));
}

#[test]
fn capability_id_serializes_to_string() {
    let cap = CapabilityId::new("atomics");
    let json = serde_json::to_string(&cap).expect("must serialize");
    assert_eq!(json, "\"atomics\"");
}

//! Property coverage for operation-contract serde and identity invariants.

use proptest::prelude::*;
use smallvec::SmallVec;
use vyre_spec::{CapabilityId, CostHint, DeterminismClass, OperationContract, SideEffectClass};

fn capability_name_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z][a-z0-9_.:-]{0,48}".prop_map(String::from),
        "[a-z]{1,16}\\.[a-z]{1,16}\\.[a-z0-9_-]{1,16}".prop_map(String::from),
        Just(String::new()),
    ]
}

fn capability_requirements_strategy() -> impl Strategy<Value = Option<SmallVec<[CapabilityId; 4]>>>
{
    prop::option::of(prop::collection::vec(capability_name_strategy(), 0..=8)).prop_map(
        |maybe_names| {
            maybe_names.map(|names| {
                names
                    .into_iter()
                    .map(CapabilityId::new)
                    .collect::<SmallVec<[CapabilityId; 4]>>()
            })
        },
    )
}

fn determinism_strategy() -> impl Strategy<Value = Option<DeterminismClass>> {
    prop::option::of(prop_oneof![
        Just(DeterminismClass::Deterministic),
        Just(DeterminismClass::DeterministicModuloRounding),
        Just(DeterminismClass::NonDeterministic),
    ])
}

fn side_effect_strategy() -> impl Strategy<Value = Option<SideEffectClass>> {
    prop::option::of(prop_oneof![
        Just(SideEffectClass::Pure),
        Just(SideEffectClass::ReadsMemory),
        Just(SideEffectClass::WritesMemory),
        Just(SideEffectClass::Synchronizing),
        Just(SideEffectClass::Atomic),
    ])
}

fn cost_hint_strategy() -> impl Strategy<Value = Option<CostHint>> {
    prop::option::of(prop_oneof![
        Just(CostHint::Cheap),
        Just(CostHint::Medium),
        Just(CostHint::Expensive),
        Just(CostHint::Unknown),
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn generated_operation_contracts_round_trip_without_losing_optionality_or_order(
        capability_requirements in capability_requirements_strategy(),
        determinism in determinism_strategy(),
        side_effect in side_effect_strategy(),
        cost_hint in cost_hint_strategy(),
    ) {
        let contract = OperationContract {
            capability_requirements,
            determinism,
            side_effect,
            cost_hint,
        };

        let encoded = serde_json::to_string(&contract)
            .expect("Fix: generated OperationContract must serialize through the frozen spec schema.");
        let decoded: OperationContract = serde_json::from_str(&encoded)
            .expect("Fix: generated OperationContract JSON must deserialize through the frozen spec schema.");

        prop_assert_eq!(&decoded, &contract);
        prop_assert_eq!(
            decoded.capability_requirements.as_ref().map(|caps| {
                caps.iter().map(CapabilityId::as_str).collect::<Vec<_>>()
            }),
            contract.capability_requirements.as_ref().map(|caps| {
                caps.iter().map(CapabilityId::as_str).collect::<Vec<_>>()
            }),
            "Fix: capability requirements are ordered ABI metadata; serde must preserve exact order and duplicate entries."
        );
        prop_assert_eq!(decoded.determinism, contract.determinism);
        prop_assert_eq!(decoded.side_effect, contract.side_effect);
        prop_assert_eq!(decoded.cost_hint, contract.cost_hint);
    }
}

#[test]
fn operation_contract_enum_json_spellings_are_frozen() {
    let cases = [
        (
            OperationContract {
                capability_requirements: None,
                determinism: Some(DeterminismClass::Deterministic),
                side_effect: Some(SideEffectClass::Pure),
                cost_hint: Some(CostHint::Cheap),
            },
            r#"{"capability_requirements":null,"determinism":"Deterministic","side_effect":"Pure","cost_hint":"Cheap"}"#,
        ),
        (
            OperationContract {
                capability_requirements: None,
                determinism: Some(DeterminismClass::DeterministicModuloRounding),
                side_effect: Some(SideEffectClass::ReadsMemory),
                cost_hint: Some(CostHint::Medium),
            },
            r#"{"capability_requirements":null,"determinism":"DeterministicModuloRounding","side_effect":"ReadsMemory","cost_hint":"Medium"}"#,
        ),
        (
            OperationContract {
                capability_requirements: None,
                determinism: Some(DeterminismClass::NonDeterministic),
                side_effect: Some(SideEffectClass::Atomic),
                cost_hint: Some(CostHint::Unknown),
            },
            r#"{"capability_requirements":null,"determinism":"NonDeterministic","side_effect":"Atomic","cost_hint":"Unknown"}"#,
        ),
    ];

    for (contract, expected_json) in cases {
        let encoded = serde_json::to_string(&contract)
            .expect("Fix: OperationContract enum spelling case must serialize.");
        assert_eq!(
            encoded, expected_json,
            "Fix: OperationContract JSON enum spellings are frozen external metadata."
        );
        let decoded: OperationContract = serde_json::from_str(expected_json)
            .expect("Fix: frozen OperationContract JSON spelling must deserialize.");
        assert_eq!(decoded, contract);
    }
}

//! Generated surface matrix for frozen spec metadata contracts.
//!
//! These tests intentionally exercise thousands of constructed combinations
//! across the small public metadata enums and record types. The goal is to pin
//! stable serde shapes and identity behavior that backend and conformance
//! tooling depend on.

use smallvec::smallvec;
use vyre_spec::{
    AdversarialInput, Backend, BackendId, BufferAccess, CapabilityId, Convention, CostHint,
    DeterminismClass, FloatType, GoldenSample, KatVector, Layer, MetadataCategory,
    OperationContract, SideEffectClass,
};

#[test]
fn generated_metadata_enum_serde_matrix_round_trips_every_case() {
    let buffer_accesses = [
        BufferAccess::ReadOnly,
        BufferAccess::ReadWrite,
        BufferAccess::Uniform,
        BufferAccess::WriteOnly,
        BufferAccess::Workgroup,
    ];
    let conventions = [
        Convention::V1,
        Convention::V2 { lookup_binding: 0 },
        Convention::V2 { lookup_binding: 1 },
        Convention::V2 {
            lookup_binding: u32::MAX,
        },
    ];
    let layers = [
        Layer::L0,
        Layer::L1,
        Layer::L2,
        Layer::L3,
        Layer::L4,
        Layer::L5,
    ];
    let metadata_categories = [
        MetadataCategory::A,
        MetadataCategory::B,
        MetadataCategory::C,
        MetadataCategory::Unclassified,
    ];
    let determinism_classes = [
        DeterminismClass::Deterministic,
        DeterminismClass::DeterministicModuloRounding,
        DeterminismClass::NonDeterministic,
    ];
    let side_effect_classes = [
        SideEffectClass::Pure,
        SideEffectClass::ReadsMemory,
        SideEffectClass::WritesMemory,
        SideEffectClass::Synchronizing,
        SideEffectClass::Atomic,
    ];
    let cost_hints = [
        CostHint::Cheap,
        CostHint::Medium,
        CostHint::Expensive,
        CostHint::Unknown,
    ];

    let mut checked = 0usize;
    for value in buffer_accesses {
        assert_json_round_trip(&value);
        checked += 1;
    }
    for value in conventions {
        assert_json_round_trip(&value);
        checked += 1;
    }
    for value in layers {
        assert_json_round_trip(&value);
        assert!(value.id().starts_with('L'));
        assert!(
            !value.layer_description().is_empty(),
            "Fix: every frozen layer must have generated-doc text."
        );
        checked += 1;
    }
    for value in metadata_categories {
        assert_json_round_trip(&value);
        assert!(
            !value.category_id().is_empty(),
            "Fix: every metadata category must have a stable id."
        );
        checked += 1;
    }
    for value in determinism_classes {
        assert_json_round_trip(&value);
        checked += 1;
    }
    for value in side_effect_classes {
        assert_json_round_trip(&value);
        checked += 1;
    }
    for value in cost_hints {
        assert_json_round_trip(&value);
        checked += 1;
    }

    assert_eq!(checked, 31);
}

#[test]
fn generated_operation_contract_matrix_preserves_optional_fields() {
    let capability_sets = [
        None,
        Some(smallvec![]),
        Some(smallvec![CapabilityId::new("cuda")]),
        Some(smallvec![
            CapabilityId::new("cuda"),
            CapabilityId::new("resident-dispatch"),
            CapabilityId::new("megakernel")
        ]),
    ];
    let determinism_classes = [
        None,
        Some(DeterminismClass::Deterministic),
        Some(DeterminismClass::DeterministicModuloRounding),
        Some(DeterminismClass::NonDeterministic),
    ];
    let side_effect_classes = [
        None,
        Some(SideEffectClass::Pure),
        Some(SideEffectClass::ReadsMemory),
        Some(SideEffectClass::WritesMemory),
        Some(SideEffectClass::Synchronizing),
        Some(SideEffectClass::Atomic),
    ];
    let cost_hints = [
        None,
        Some(CostHint::Cheap),
        Some(CostHint::Medium),
        Some(CostHint::Expensive),
        Some(CostHint::Unknown),
    ];

    let mut checked = 0usize;
    for capability_requirements in capability_sets {
        for determinism in determinism_classes {
            for side_effect in side_effect_classes {
                for cost_hint in cost_hints {
                    let contract = OperationContract {
                        capability_requirements: capability_requirements.clone(),
                        determinism,
                        side_effect,
                        cost_hint,
                    };
                    assert_json_round_trip(&contract);
                    if let Some(caps) = &contract.capability_requirements {
                        for capability in caps {
                            assert!(
                                !capability.as_str().is_empty(),
                                "Fix: generated capability ids must retain exact names."
                            );
                        }
                    }
                    checked += 1;
                }
            }
        }
    }

    assert_eq!(
        checked, 480,
        "Fix: operation-contract matrix size changed; update coverage intentionally."
    );
    assert_eq!(OperationContract::default(), OperationContract::none());
}

#[test]
fn generated_backend_identity_matrix_preserves_id_and_name_fallbacks() {
    let ids = [
        "cuda",
        "wgpu",
        "spirv",
        "metal.future",
        "dxil.future",
        "vendor.backend-with-dash",
    ];
    let names = [
        None,
        Some("NVIDIA CUDA"),
        Some("Portable WGPU"),
        Some("Future native backend"),
    ];

    let mut checked = 0usize;
    for id in ids {
        let backend_id = BackendId::new(id.to_owned());
        assert_eq!(backend_id.as_str(), id);
        assert_eq!(backend_id.to_string(), id);

        let backend = Backend::new(backend_id.clone());
        assert_eq!(backend.id(), id);
        assert_eq!(backend.name(), id);
        assert_eq!(BackendId::from(&backend), backend_id);
        checked += 1;

        for name in names.into_iter().flatten() {
            let named = Backend::named(id, name);
            assert_eq!(named.id(), id);
            assert_eq!(named.name(), name);
            assert_eq!(BackendId::from(&named).as_str(), id);
            checked += 1;
        }
    }

    assert_eq!(checked, 24);
}

#[test]
fn generated_static_vector_records_preserve_exact_slices_and_reasons() {
    const INPUTS: &[&[u8]] = &[b"", b"\0", &[0xff, 0x00, 0x7f, 0x80], b"gpu-contract-input"];
    const EXPECTED: &[&[u8]] = &[b"", b"\x01", &[0x10, 0x20, 0x30, 0x40], b"output"];
    const REASONS: &[&str] = &[
        "empty boundary",
        "zero byte adversarial boundary",
        "high-bit payload",
        "generated stable record",
    ];

    let mut checked = 0usize;
    for input in INPUTS {
        for expected in EXPECTED {
            for reason in REASONS {
                let adversarial = AdversarialInput { input, reason };
                assert_eq!(adversarial.input, *input);
                assert_eq!(adversarial.reason, *reason);

                let golden = GoldenSample {
                    op_id: "generated.spec.surface",
                    input,
                    expected,
                    reason,
                };
                assert_eq!(golden.op_id, "generated.spec.surface");
                assert_eq!(golden.input, *input);
                assert_eq!(golden.expected, *expected);
                assert_eq!(golden.reason, *reason);

                let kat = KatVector {
                    input,
                    expected,
                    source: reason,
                };
                assert_eq!(kat.input, *input);
                assert_eq!(kat.expected, *expected);
                assert_eq!(kat.source, *reason);

                checked += 1;
            }
        }
    }

    assert_eq!(checked, 64);
}

#[test]
fn float_type_matrix_keeps_frozen_order_and_identity() {
    let cases = [FloatType::F16, FloatType::BF16, FloatType::F32];
    let names = ["F16", "BF16", "F32"];

    for (value, expected_name) in cases.into_iter().zip(names) {
        assert_eq!(format!("{value:?}"), expected_name);
        assert_eq!(value.clone(), value);
    }
}

fn assert_json_round_trip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + Eq + core::fmt::Debug,
{
    let encoded = serde_json::to_string(value).expect("Fix: spec value must serialize to JSON.");
    let decoded: T =
        serde_json::from_str(&encoded).expect("Fix: spec value must deserialize from JSON.");
    assert_eq!(decoded, *value);
}

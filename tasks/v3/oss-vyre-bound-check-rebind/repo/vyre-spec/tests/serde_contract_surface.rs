//! Freeze tests for serde and byte-accounting contracts in the public spec API.
//!
//! These tests pin the data-only boundary consumed by backends and conformance
//! runners: every public spec record must round-trip without losing optional
//! contract metadata, and missing optional fields must deserialize to `None`
//! rather than inventing implicit requirements.

use vyre_spec::op_signature::SignatureParam;
use vyre_spec::{
    CapabilityId, CostHint, DataType, DeterminismClass, OpSignature, OperationContract,
    SideEffectClass, TypeId,
};

#[test]
fn data_type_round_trip_preserves_nested_shape_and_extension_handles() {
    let cases = [
        DataType::U8,
        DataType::I16,
        DataType::U32,
        DataType::F64,
        DataType::Handle(TypeId(0xCAFE_BABE)),
        DataType::Vec {
            element: Box::new(DataType::F32),
            count: 4,
        },
        DataType::TensorShaped {
            element: Box::new(DataType::I4),
            shape: [2, 4, 8].as_slice().into(),
        },
        DataType::SparseCsr {
            element: Box::new(DataType::F8E4M3),
        },
        DataType::SparseBsr {
            element: Box::new(DataType::NF4),
            block_rows: 16,
            block_cols: 32,
        },
        DataType::DeviceMesh {
            axes: [2, 2, 4].as_slice().into(),
        },
    ];

    for ty in cases {
        let encoded = serde_json::to_string(&ty).expect("DataType must serialize");
        let decoded: DataType = serde_json::from_str(&encoded).expect("DataType must deserialize");
        assert_eq!(decoded, ty, "DataType serde round-trip changed {encoded}");
    }
}

#[test]
fn data_type_rejects_unknown_serde_variant() {
    let err = serde_json::from_str::<DataType>(r#""DefinitelyNotAType""#)
        .expect_err("unknown DataType variants must not deserialize");
    assert!(
        err.to_string().contains("unknown variant"),
        "unexpected serde error: {err}"
    );
}

#[test]
fn op_signature_default_fields_deserialize_to_absent_metadata() {
    let json = r#"{"inputs":["U32","F32"],"output":"Bool"}"#;
    let signature: OpSignature =
        serde_json::from_str(json).expect("minimal OpSignature must deserialize");

    assert_eq!(signature.inputs, vec![DataType::U32, DataType::F32]);
    assert_eq!(signature.output, DataType::Bool);
    assert!(signature.input_params.is_none());
    assert!(signature.output_params.is_none());
    assert!(signature.contract.is_none());
    assert_eq!(signature.min_input_bytes(), 8);
}

#[test]
fn op_signature_round_trip_preserves_typed_params_and_contract() {
    let signature = OpSignature {
        inputs: vec![
            DataType::Vec {
                element: Box::new(DataType::U16),
                count: 8,
            },
            DataType::Bytes,
        ],
        output: DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape: [1, 1024].as_slice().into(),
        },
        input_params: Some(vec![
            SignatureParam {
                name: "lanes".to_owned(),
                ty: DataType::Vec {
                    element: Box::new(DataType::U16),
                    count: 8,
                },
                metadata: Some("packed input lanes".to_owned()),
            },
            SignatureParam {
                name: "payload".to_owned(),
                ty: DataType::Bytes,
                metadata: None,
            },
        ]),
        output_params: Some(vec![SignatureParam {
            name: "tensor".to_owned(),
            ty: DataType::Tensor,
            metadata: Some("runtime-shaped output".to_owned()),
        }]),
        contract: Some(OperationContract {
            capability_requirements: Some(
                [
                    CapabilityId::new("cuda.subgroup"),
                    CapabilityId::new("runtime.megakernel"),
                ]
                .into_iter()
                .collect(),
            ),
            determinism: Some(DeterminismClass::DeterministicModuloRounding),
            side_effect: Some(SideEffectClass::ReadsMemory),
            cost_hint: Some(CostHint::Expensive),
        }),
    };

    let encoded = serde_json::to_string(&signature).expect("OpSignature must serialize");
    let decoded: OpSignature =
        serde_json::from_str(&encoded).expect("OpSignature must deserialize");

    assert_eq!(decoded, signature);
    assert_eq!(decoded.min_input_bytes(), 16);
}

#[test]
fn operation_contract_missing_fields_deserialize_to_no_requirements() {
    let contract: OperationContract =
        serde_json::from_str("{}").expect("empty OperationContract must deserialize");

    assert_eq!(contract, OperationContract::none());
    assert!(contract.capability_requirements.is_none());
    assert!(contract.determinism.is_none());
    assert!(contract.side_effect.is_none());
    assert!(contract.cost_hint.is_none());
}

#[test]
fn capability_id_round_trip_preserves_exact_name() {
    let capability = CapabilityId::new("cuda.graph.resident-csr");
    let encoded = serde_json::to_string(&capability).expect("CapabilityId must serialize");
    let decoded: CapabilityId =
        serde_json::from_str(&encoded).expect("CapabilityId must deserialize");

    assert_eq!(decoded, capability);
    assert_eq!(decoded.as_str(), "cuda.graph.resident-csr");
}

#[test]
fn type_id_exposes_exact_raw_value() {
    assert_eq!(TypeId(0).as_u32(), 0);
    assert_eq!(TypeId(u32::MAX).as_u32(), u32::MAX);
}

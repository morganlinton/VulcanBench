//! Generated coverage for operation signatures that carry execution contracts.
//!
//! `OpSignature` is the frozen handoff between spec, conformance runners, and
//! backend vendors. This matrix pins that attaching `OperationContract`
//! metadata never changes byte accounting, never drops capability ordering, and
//! remains serde-stable across thousands of generated type/contract shapes.

use smallvec::smallvec;
use vyre_spec::op_signature::SignatureParam;
use vyre_spec::{
    CapabilityId, CostHint, DataType, DeterminismClass, OpSignature, OperationContract,
    QuantizationScale, QuantizationZeroPoint, SideEffectClass, TypeId,
};

#[test]
fn generated_op_signatures_with_contracts_round_trip_for_8192_cases() {
    let mut checked = 0usize;
    for seed in 0u64..8192 {
        let inputs = generated_inputs(seed);
        let output = generated_type(seed ^ 0xfeed_face_cafe_babe);
        let input_params = generated_params(seed, &inputs);
        let output_params = generated_params(seed.rotate_left(17), std::slice::from_ref(&output));
        let contract = generated_contract(seed);
        let expected_min_input_bytes = inputs.iter().map(DataType::min_bytes).sum::<usize>();
        let expected_caps = contract
            .capability_requirements
            .as_ref()
            .map(|caps| {
                caps.iter()
                    .map(|capability| capability.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let signature = OpSignature {
            inputs,
            output,
            input_params: Some(input_params),
            output_params: Some(output_params),
            contract: Some(contract),
        };

        assert_eq!(
            signature.min_input_bytes(),
            expected_min_input_bytes,
            "case {seed}: contract metadata must not affect input byte accounting"
        );

        let json = serde_json::to_string(&signature)
            .expect("Fix: contracted OpSignature must serialize through the frozen spec API.");
        let decoded: OpSignature = serde_json::from_str(&json).expect(
            "Fix: contracted OpSignature JSON must deserialize through the frozen spec API.",
        );

        assert_eq!(decoded, signature, "case {seed}: serde round-trip drift");
        assert_eq!(
            decoded.min_input_bytes(),
            expected_min_input_bytes,
            "case {seed}: decoded byte accounting drift"
        );
        let decoded_caps = decoded
            .contract
            .as_ref()
            .and_then(|contract| contract.capability_requirements.as_ref())
            .map(|caps| {
                caps.iter()
                    .map(|capability| capability.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(
            decoded_caps, expected_caps,
            "case {seed}: capability order is part of the frozen contract"
        );
        checked += 1;
    }
    assert_eq!(checked, 8192);
}

#[test]
fn operation_contract_missing_json_fields_default_to_none() {
    let decoded: OperationContract = serde_json::from_str("{}")
        .expect("Fix: empty OperationContract JSON must deserialize with default fields.");
    assert_eq!(decoded, OperationContract::none());

    let partial: OperationContract = serde_json::from_str(r#"{"capability_requirements":[]}"#)
        .expect("Fix: explicit empty capability list must deserialize.");
    assert!(partial
        .capability_requirements
        .as_ref()
        .is_some_and(|capabilities| capabilities.is_empty()));
    assert_eq!(partial.determinism, None);
    assert_eq!(partial.side_effect, None);
    assert_eq!(partial.cost_hint, None);
}

fn generated_inputs(seed: u64) -> Vec<DataType> {
    let len = (seed as usize % 8) + 1;
    (0..len)
        .map(|index| generated_type(next_state(seed ^ index as u64)))
        .collect()
}

fn generated_params(seed: u64, types: &[DataType]) -> Vec<SignatureParam> {
    types
        .iter()
        .enumerate()
        .map(|(index, ty)| SignatureParam {
            name: format!("p{index}_{}", seed.rotate_left((index & 63) as u32) & 0xff),
            ty: ty.clone(),
            metadata: ((seed >> (index & 15)) & 1 == 1)
                .then(|| format!("generated role {index} seed {seed}")),
        })
        .collect()
}

fn generated_contract(seed: u64) -> OperationContract {
    OperationContract {
        capability_requirements: Some(match seed % 5 {
            0 => smallvec![],
            1 => smallvec![CapabilityId::new("cuda")],
            2 => smallvec![
                CapabilityId::new("cuda"),
                CapabilityId::new("resident_dispatch"),
            ],
            3 => smallvec![
                CapabilityId::new("cuda_graph"),
                CapabilityId::new("cooperative_launch"),
                CapabilityId::new("async_copy"),
            ],
            _ => smallvec![
                CapabilityId::new("multi_gpu"),
                CapabilityId::new("collectives"),
                CapabilityId::new("quantized_tensor_cores"),
                CapabilityId::new("gpudirect_storage"),
            ],
        }),
        determinism: Some(match (seed >> 3) % 3 {
            0 => DeterminismClass::Deterministic,
            1 => DeterminismClass::DeterministicModuloRounding,
            _ => DeterminismClass::NonDeterministic,
        }),
        side_effect: Some(match (seed >> 7) % 5 {
            0 => SideEffectClass::Pure,
            1 => SideEffectClass::ReadsMemory,
            2 => SideEffectClass::WritesMemory,
            3 => SideEffectClass::Synchronizing,
            _ => SideEffectClass::Atomic,
        }),
        cost_hint: Some(match (seed >> 11) % 4 {
            0 => CostHint::Cheap,
            1 => CostHint::Medium,
            2 => CostHint::Expensive,
            _ => CostHint::Unknown,
        }),
    }
}

fn generated_type(seed: u64) -> DataType {
    match seed % 28 {
        0 => DataType::U8,
        1 => DataType::U16,
        2 => DataType::U32,
        3 => DataType::U64,
        4 => DataType::I8,
        5 => DataType::I16,
        6 => DataType::I32,
        7 => DataType::I64,
        8 => DataType::Bool,
        9 => DataType::F16,
        10 => DataType::BF16,
        11 => DataType::F32,
        12 => DataType::F64,
        13 => DataType::F8E4M3,
        14 => DataType::F8E5M2,
        15 => DataType::I4,
        16 => DataType::FP4,
        17 => DataType::NF4,
        18 => DataType::Vec2U32,
        19 => DataType::Vec4U32,
        20 => DataType::Bytes,
        21 => DataType::Tensor,
        22 => DataType::Handle(TypeId((seed >> 8) as u32)),
        23 => DataType::Array {
            element_size: ((seed >> 13) as usize % 64) + 1,
        },
        24 => DataType::Vec {
            element: Box::new(DataType::U32),
            count: ((seed >> 17) as u8 % 16) + 1,
        },
        25 => DataType::SparseCsr {
            element: Box::new(DataType::F32),
        },
        26 => DataType::DeviceMesh {
            axes: [((seed >> 19) as u32 % 8) + 1, ((seed >> 23) as u32 % 8) + 1]
                .as_slice()
                .into(),
        },
        _ => DataType::Quantized {
            storage: Box::new(match (seed >> 29) % 5 {
                0 => DataType::I4,
                1 => DataType::FP4,
                2 => DataType::NF4,
                3 => DataType::U8,
                _ => DataType::I8,
            }),
            scale: QuantizationScale::PerGroup {
                group_size: ((seed >> 31) as u32 % 256) + 1,
            },
            zero_point: match (seed >> 39) % 3 {
                0 => QuantizationZeroPoint::Absent,
                1 => QuantizationZeroPoint::PerTensor,
                _ => QuantizationZeroPoint::PerGroup {
                    group_size: ((seed >> 41) as u32 % 256) + 1,
                },
            },
        },
    }
}

fn next_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

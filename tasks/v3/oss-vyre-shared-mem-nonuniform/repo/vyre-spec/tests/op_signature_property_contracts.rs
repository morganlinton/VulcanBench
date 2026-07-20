//! Generated property coverage for operation signature byte accounting.

use proptest::prelude::*;
use vyre_spec::op_signature::SignatureParam;
use vyre_spec::{DataType, OpSignature, TypeId};

fn scalar_type_strategy() -> impl Strategy<Value = DataType> {
    prop_oneof![
        Just(DataType::U8),
        Just(DataType::U16),
        Just(DataType::U32),
        Just(DataType::U64),
        Just(DataType::I8),
        Just(DataType::I16),
        Just(DataType::I32),
        Just(DataType::I64),
        Just(DataType::Bool),
        Just(DataType::F16),
        Just(DataType::BF16),
        Just(DataType::F32),
        Just(DataType::F64),
        Just(DataType::F8E4M3),
        Just(DataType::F8E5M2),
        Just(DataType::I4),
        Just(DataType::FP4),
        Just(DataType::NF4),
        Just(DataType::Vec2U32),
        Just(DataType::Vec4U32),
        Just(DataType::Bytes),
        Just(DataType::Tensor),
        any::<u32>().prop_map(|raw| DataType::Handle(TypeId(raw))),
        (1usize..=64usize).prop_map(|element_size| DataType::Array { element_size }),
    ]
}

fn data_type_strategy() -> BoxedStrategy<DataType> {
    scalar_type_strategy()
        .prop_recursive(3, 48, 4, |inner| {
            prop_oneof![
                (inner.clone(), 1u8..=16u8).prop_map(|(element, count)| DataType::Vec {
                    element: Box::new(element),
                    count,
                }),
                (inner.clone(), prop::collection::vec(1u32..=16, 0..=4)).prop_map(
                    |(element, shape)| DataType::TensorShaped {
                        element: Box::new(element),
                        shape: shape.as_slice().into(),
                    },
                ),
                inner.clone().prop_map(|element| DataType::SparseCsr {
                    element: Box::new(element),
                }),
                inner.prop_map(|element| DataType::SparseCoo {
                    element: Box::new(element),
                }),
            ]
        })
        .boxed()
}

fn signature_param_strategy() -> impl Strategy<Value = SignatureParam> {
    (
        "[a-z][a-z0-9_]{0,24}",
        data_type_strategy(),
        prop::option::of("[a-zA-Z0-9 _./:-]{0,48}"),
    )
        .prop_map(|(name, ty, metadata)| SignatureParam { name, ty, metadata })
}

fn optional_params_strategy() -> impl Strategy<Value = Option<Vec<SignatureParam>>> {
    prop::option::of(prop::collection::vec(signature_param_strategy(), 0..=6))
}

proptest! {
    #[test]
    fn generated_signatures_round_trip_and_preserve_byte_accounting(
        inputs in prop::collection::vec(data_type_strategy(), 0..=8),
        output in data_type_strategy(),
        input_params in optional_params_strategy(),
        output_params in optional_params_strategy(),
    ) {
        let expected_min_input_bytes = inputs.iter().map(DataType::min_bytes).sum::<usize>();
        let signature = OpSignature {
            inputs,
            output,
            input_params,
            output_params,
            contract: None,
        };

        prop_assert_eq!(signature.min_input_bytes(), expected_min_input_bytes);

        let encoded = serde_json::to_string(&signature)
            .expect("Fix: generated OpSignature must serialize through the frozen spec contract");
        let decoded: OpSignature = serde_json::from_str(&encoded)
            .expect("Fix: generated OpSignature JSON must deserialize through the frozen spec contract");

        prop_assert_eq!(decoded.min_input_bytes(), expected_min_input_bytes);
        prop_assert_eq!(decoded, signature);
    }
}

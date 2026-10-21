//! Generated property coverage for `DataType` layout and serde contracts.

use proptest::prelude::*;
use vyre_spec::extension::ExtensionDataTypeId;
use vyre_spec::{DataType, QuantizationScale, QuantizationZeroPoint, TypeId};

fn data_type_strategy() -> BoxedStrategy<DataType> {
    let leaf = prop_oneof![
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
        "[a-z][a-z0-9_.-]{0,48}"
            .prop_map(|name| DataType::Opaque(ExtensionDataTypeId::from_name(&name))),
        prop::collection::vec(1u32..=16, 1..=3).prop_map(|axes| DataType::DeviceMesh {
            axes: axes.as_slice().into()
        }),
    ];

    leaf.prop_recursive(3, 64, 4, |inner| {
        let scale = prop_oneof![
            Just(QuantizationScale::PerTensor),
            (0u32..=4u32).prop_map(|axis| QuantizationScale::PerChannel { axis }),
            (1u32..=256u32).prop_map(|group_size| QuantizationScale::PerGroup { group_size }),
        ];
        let zero_point = prop_oneof![
            Just(QuantizationZeroPoint::Absent),
            Just(QuantizationZeroPoint::PerTensor),
            (0u32..=4u32).prop_map(|axis| QuantizationZeroPoint::PerChannel { axis }),
            (1u32..=256u32).prop_map(|group_size| QuantizationZeroPoint::PerGroup { group_size }),
        ];
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
            inner.clone().prop_map(|element| DataType::SparseCoo {
                element: Box::new(element),
            }),
            (inner.clone(), 1u32..=16u32, 1u32..=16u32).prop_map(
                |(element, block_rows, block_cols)| DataType::SparseBsr {
                    element: Box::new(element),
                    block_rows,
                    block_cols,
                },
            ),
            (
                prop_oneof![
                    Just(DataType::I4),
                    Just(DataType::I8),
                    Just(DataType::I16),
                    Just(DataType::U8),
                    Just(DataType::U16),
                    Just(DataType::F8E4M3),
                    Just(DataType::F8E5M2),
                    Just(DataType::FP4),
                    Just(DataType::NF4),
                ],
                scale,
                zero_point,
            )
                .prop_map(|(storage, scale, zero_point)| DataType::Quantized {
                    storage: Box::new(storage),
                    scale,
                    zero_point,
                }),
        ]
    })
    .boxed()
}

proptest! {
    #[test]
    fn generated_data_types_round_trip_through_json(ty in data_type_strategy()) {
        let encoded = serde_json::to_string(&ty)
            .expect("Fix: generated DataType must serialize through the frozen spec contract");
        let decoded: DataType = serde_json::from_str(&encoded)
            .expect("Fix: generated DataType JSON must deserialize through the frozen spec contract");

        prop_assert_eq!(decoded, ty);
    }

    #[test]
    fn generated_data_type_layout_bounds_are_coherent(ty in data_type_strategy()) {
        if let Some(max_bytes) = ty.max_bytes() {
            prop_assert!(
                ty.min_bytes() <= max_bytes,
                "Fix: min_bytes must never exceed max_bytes for {ty}: min={} max={max_bytes}",
                ty.min_bytes()
            );
        }

        if let (Some(bit_width), Some(size_bytes)) = (ty.bit_width(), ty.size_bytes()) {
            prop_assert!(
                size_bytes.saturating_mul(8) >= bit_width,
                "Fix: size_bytes must have enough bits for {ty}: size={size_bytes}, bits={bit_width}"
            );
        }

        prop_assert!(!ty.to_string().is_empty(), "Fix: DataType display must never be empty");
        prop_assert!(
            ty.validate_layout().is_ok(),
            "Fix: generated valid DataType strategy produced malformed layout metadata for {ty}"
        );
    }

    #[test]
    fn generated_quantized_datatypes_preserve_storage_width(
        storage in prop_oneof![
            Just(DataType::I4),
            Just(DataType::I8),
            Just(DataType::I16),
            Just(DataType::U8),
            Just(DataType::U16),
            Just(DataType::F8E4M3),
            Just(DataType::F8E5M2),
            Just(DataType::FP4),
            Just(DataType::NF4),
        ],
        group_size in 1u32..=512,
    ) {
        let ty = DataType::Quantized {
            storage: Box::new(storage.clone()),
            scale: QuantizationScale::PerGroup { group_size },
            zero_point: QuantizationZeroPoint::PerGroup { group_size },
        };

        prop_assert!(ty.is_quantized());
        prop_assert!(storage.is_quantized_storage());
        prop_assert_eq!(ty.bit_width(), storage.bit_width());
        prop_assert_eq!(ty.size_bytes(), storage.size_bytes());
        prop_assert!(!ty.is_float_family());
    }
}

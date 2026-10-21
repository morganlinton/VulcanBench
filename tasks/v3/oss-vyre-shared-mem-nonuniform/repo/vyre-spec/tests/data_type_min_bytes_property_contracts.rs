//! Property gates for `DataType::min_bytes` / `max_bytes` consistency.

use proptest::prelude::*;
use vyre_spec::{DataType, QuantizationScale, QuantizationZeroPoint};

fn leaf_strategy() -> impl Strategy<Value = DataType> {
    prop_oneof![
        Just(DataType::U8),
        Just(DataType::U16),
        Just(DataType::U32),
        Just(DataType::I32),
        Just(DataType::Bool),
        Just(DataType::F32),
        Just(DataType::I4),
        Just(DataType::NF4),
        Just(DataType::Quantized {
            storage: Box::new(DataType::I4),
            scale: QuantizationScale::PerGroup { group_size: 128 },
            zero_point: QuantizationZeroPoint::Absent,
        }),
        Just(DataType::Vec2U32),
        Just(DataType::Vec4U32),
        (1u8..=8).prop_map(|count| DataType::Vec {
            element: Box::new(DataType::U32),
            count,
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn min_bytes_le_max_bytes_for_fixed_width(ty in leaf_strategy()) {
        let min = ty.min_bytes();
        let max = ty.max_bytes().unwrap_or(min);
        prop_assert!(min <= max);
    }

    #[test]
    fn vec_min_bytes_scales_with_count(count in 1u8..=16) {
        let ty = DataType::Vec {
            element: Box::new(DataType::U32),
            count,
        };
        prop_assert_eq!(ty.min_bytes(), 4usize * count as usize);
    }

    #[test]
    fn opaque_types_have_zero_min_bytes(name in "[a-z][a-z0-9_]{0,20}") {
        use vyre_spec::extension::ExtensionDataTypeId;
        let ty = DataType::Opaque(ExtensionDataTypeId::from_name(&name));
        prop_assert_eq!(ty.min_bytes(), 0);
        prop_assert!(ty.builtin_wire_tag().is_none());
    }
}

#[test]
fn builtin_wire_tag_implies_positive_min_bytes() {
    let ty = DataType::F64;
    assert!(ty.builtin_wire_tag().is_some());
    assert!(ty.min_bytes() > 0);
}

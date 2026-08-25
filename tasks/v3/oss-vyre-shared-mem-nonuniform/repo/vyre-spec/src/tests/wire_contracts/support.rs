use crate::{DataType, QuantizationScale, QuantizationZeroPoint, TypeId};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;

pub(super) fn data_type_wire_cases() -> Vec<(&'static str, DataType, u8)> {
    vec![
        ("U32", DataType::U32, 0x01),
        ("I32", DataType::I32, 0x02),
        ("U64", DataType::U64, 0x03),
        ("Vec2U32", DataType::Vec2U32, 0x04),
        ("Vec4U32", DataType::Vec4U32, 0x05),
        ("Bool", DataType::Bool, 0x06),
        ("Bytes", DataType::Bytes, 0x07),
        ("Array", DataType::Array { element_size: 4 }, 0x08),
        ("F16", DataType::F16, 0x09),
        ("BF16", DataType::BF16, 0x0A),
        ("F32", DataType::F32, 0x0B),
        ("F64", DataType::F64, 0x0C),
        ("Tensor", DataType::Tensor, 0x0D),
        ("U8", DataType::U8, 0x0E),
        ("U16", DataType::U16, 0x0F),
        ("I8", DataType::I8, 0x10),
        ("I16", DataType::I16, 0x11),
        ("I64", DataType::I64, 0x12),
        ("Handle", DataType::Handle(TypeId(9)), 0x13),
        (
            "Vec",
            DataType::Vec {
                element: Box::new(DataType::U16),
                count: 8,
            },
            0x14,
        ),
        (
            "TensorShaped",
            DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: smallvec::smallvec![4, 16, 32],
            },
            0x15,
        ),
        (
            "SparseCsr",
            DataType::SparseCsr {
                element: Box::new(DataType::F32),
            },
            0x16,
        ),
        (
            "SparseCoo",
            DataType::SparseCoo {
                element: Box::new(DataType::I32),
            },
            0x17,
        ),
        (
            "SparseBsr",
            DataType::SparseBsr {
                element: Box::new(DataType::I4),
                block_rows: 8,
                block_cols: 8,
            },
            0x18,
        ),
        ("F8E4M3", DataType::F8E4M3, 0x19),
        ("F8E5M2", DataType::F8E5M2, 0x1A),
        ("I4", DataType::I4, 0x1B),
        ("FP4", DataType::FP4, 0x1C),
        ("NF4", DataType::NF4, 0x1D),
        (
            "DeviceMesh",
            DataType::DeviceMesh {
                axes: smallvec::smallvec![2, 4],
            },
            0x1E,
        ),
        (
            "Quantized",
            DataType::Quantized {
                storage: Box::new(DataType::I8),
                scale: QuantizationScale::PerChannel { axis: 0 },
                zero_point: QuantizationZeroPoint::PerGroup { group_size: 32 },
            },
            0x1F,
        ),
    ]
}

pub(super) fn assert_payload_independent_tag(name: &str, values: &[DataType], expected: u8) {
    for value in values {
        assert_eq!(
            value.builtin_wire_tag(),
            Some(expected),
            "Fix: DataType::{name} tag must not depend on payload fields"
        );
    }
}

pub(super) fn assert_pairwise_unique_tags(family: &str, tags: &[(&'static str, u8)]) {
    let mut seen = BTreeMap::new();
    for (left_index, (left_name, left_tag)) in tags.iter().enumerate() {
        assert!(
            (0x01..=0x7F).contains(left_tag),
            "Fix: {family}::{left_name} uses non-builtin tag {left_tag:#04x}"
        );
        assert_eq!(
            seen.insert(*left_tag, *left_name),
            None,
            "Fix: {family}::{left_name} duplicates wire tag {left_tag:#04x}"
        );
        for (right_index, (right_name, right_tag)) in tags.iter().enumerate() {
            if left_index == right_index {
                assert_eq!(left_tag, right_tag);
            } else {
                assert_ne!(
                    left_tag, right_tag,
                    "Fix: {family}::{left_name} and {family}::{right_name} share wire tag {left_tag:#04x}"
                );
            }
        }
    }
}

pub(super) fn assert_json_roundtrip<T>(value: T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let encoded = serde_json::to_string(&value)
        .expect("Fix: representative frozen contract value must serialize to JSON");
    let decoded = serde_json::from_str::<T>(&encoded)
        .expect("Fix: representative frozen contract value must deserialize from JSON");
    assert_eq!(
        decoded, value,
        "Fix: JSON round-trip drifted for representative contract value"
    );
}

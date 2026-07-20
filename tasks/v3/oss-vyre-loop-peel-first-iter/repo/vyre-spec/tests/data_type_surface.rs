//! Surface tests for `DataType` enum.
//!
//! Covers variant distinctness, size calculations, and float-family detection.

use vyre_spec::data_type::{DataType, QuantizationScale, QuantizationZeroPoint};

#[test]
fn all_scalar_variants_are_distinct() {
    assert_ne!(DataType::U8, DataType::U16);
    assert_ne!(DataType::U16, DataType::U32);
    assert_ne!(DataType::U32, DataType::U64);
    assert_ne!(DataType::I8, DataType::I16);
    assert_ne!(DataType::I16, DataType::I32);
    assert_ne!(DataType::I32, DataType::I64);
    assert_ne!(DataType::F16, DataType::F32);
    assert_ne!(DataType::F32, DataType::F64);
    assert_ne!(DataType::Bool, DataType::U32);
}

#[test]
fn u32_size_is_4_bytes() {
    assert_eq!(DataType::U32.size_bytes(), Some(4));
}

#[test]
fn f32_size_is_4_bytes() {
    assert_eq!(DataType::F32.size_bytes(), Some(4));
}

#[test]
fn f64_size_is_8_bytes() {
    assert_eq!(DataType::F64.size_bytes(), Some(8));
}

#[test]
fn f32_is_float_family() {
    assert!(DataType::F32.is_float_family());
}

#[test]
fn f16_is_float_family() {
    assert!(DataType::F16.is_float_family());
}

#[test]
fn bf16_is_float_family() {
    assert!(DataType::BF16.is_float_family());
}

#[test]
fn u32_is_not_float_family() {
    assert!(!DataType::U32.is_float_family());
}

#[test]
fn bool_is_not_float_family() {
    assert!(!DataType::Bool.is_float_family());
}

#[test]
fn u32_element_size_is_none() {
    assert_eq!(DataType::U32.element_size(), None);
}

#[test]
fn vec2_u32_size_is_8() {
    assert_eq!(DataType::Vec2U32.size_bytes(), Some(8));
}

#[test]
fn vec4_u32_size_is_16() {
    assert_eq!(DataType::Vec4U32.size_bytes(), Some(16));
}

#[test]
fn min_bytes_for_u32_is_4() {
    assert_eq!(DataType::U32.min_bytes(), 4);
}

#[test]
fn max_bytes_for_scalar_is_some() {
    assert_eq!(DataType::U32.max_bytes(), Some(4));
}

#[test]
fn i64_size_is_8() {
    assert_eq!(DataType::I64.size_bytes(), Some(8));
}

#[test]
fn f8e4m3_is_float_family() {
    assert!(DataType::F8E4M3.is_float_family());
}

#[test]
fn f8e5m2_is_float_family() {
    assert!(DataType::F8E5M2.is_float_family());
}

#[test]
fn bytes_has_no_element_size() {
    assert_eq!(DataType::Bytes.element_size(), None);
}

#[test]
fn bytes_size_is_1() {
    assert_eq!(DataType::Bytes.size_bytes(), Some(1));
}

#[test]
fn tensor_has_no_size_bytes() {
    assert_eq!(DataType::Tensor.size_bytes(), None);
}

#[test]
fn array_size_roundtrips() {
    let dt = DataType::Array { element_size: 16 };
    assert_eq!(dt.element_size(), Some(16));
}

#[test]
fn quantized_int4_grouped_type_preserves_storage_layout() {
    let dt = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 128 },
        zero_point: QuantizationZeroPoint::Absent,
    };

    assert!(dt.is_quantized());
    assert_eq!(dt.builtin_wire_tag(), Some(0x1F));
    assert_eq!(dt.bit_width(), Some(4));
    assert_eq!(dt.size_bytes(), Some(1));
    assert_eq!(dt.min_bytes(), 1);
    assert!(!dt.is_float_family());
    assert_eq!(
        dt.to_string(),
        "quantized<i4;scale:per_group(size=128);zp:absent>"
    );
}

#[test]
fn quantized_storage_predicate_is_restricted_to_packed_numeric_storage() {
    for supported in [
        DataType::I4,
        DataType::I8,
        DataType::I16,
        DataType::U8,
        DataType::U16,
        DataType::F8E4M3,
        DataType::F8E5M2,
        DataType::FP4,
        DataType::NF4,
    ] {
        assert!(supported.is_quantized_storage(), "{supported}");
    }

    for unsupported in [
        DataType::U32,
        DataType::F32,
        DataType::Bool,
        DataType::Bytes,
    ] {
        assert!(!unsupported.is_quantized_storage(), "{unsupported}");
    }
}

#[test]
fn quantized_grouped_zero_point_display_is_stable() {
    let dt = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 64 },
        zero_point: QuantizationZeroPoint::PerGroup { group_size: 64 },
    };

    assert_eq!(
        dt.to_string(),
        "quantized<i4;scale:per_group(size=64);zp:per_group(size=64)>"
    );
}

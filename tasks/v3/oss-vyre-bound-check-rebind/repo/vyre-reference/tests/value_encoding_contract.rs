//! Adversarial contracts for reference-oracle value encoding.
//!
//! `Value` is the byte boundary between host inputs, the CPU oracle, and
//! backend parity comparison. These tests pin truncation, padding, truthiness,
//! and bit-exact float equality so backend mismatches cannot be hidden by
//! lossy conversion behavior.

use std::sync::Arc;

use vyre::ir::DataType;
use vyre_reference::value::Value;

#[test]
fn scalar_to_bytes_width_pads_and_truncates_little_endian() {
    assert_eq!(
        Value::U32(0xAABB_CCDD).to_bytes_width(8),
        vec![0xDD, 0xCC, 0xBB, 0xAA, 0, 0, 0, 0]
    );
    assert_eq!(
        Value::U64(0x1122_3344_5566_7788).to_bytes_width(4),
        vec![0x88, 0x77, 0x66, 0x55]
    );
    assert_eq!(Value::Bool(true).to_bytes_width(2), vec![1, 0]);
}

#[test]
fn extend_bytes_width_matches_allocating_encoding() {
    let values = [
        Value::U32(0x0102_0304),
        Value::I32(-2),
        Value::U64(0xAABB_CCDD_EEFF_0011),
        Value::Bool(false),
        Value::Bytes(Arc::from([9, 8, 7, 6, 5])),
        Value::Float(-0.0),
        Value::Array(vec![Value::U32(1), Value::Bool(true)]),
    ];

    for value in values {
        for width in [0, 1, 4, 8, 12] {
            let mut extended = Vec::new();
            value
                .extend_bytes_width(width, &mut extended)
                .expect("reference value byte extension must not fail for bounded fixtures");
            assert_eq!(
                extended,
                value.to_bytes_width(width),
                "{value:?} width {width}"
            );
        }
    }
}

#[test]
fn numeric_narrowing_rejects_negative_and_oversized_values() {
    assert_eq!(Value::I32(-1).try_as_u32(), None);
    assert_eq!(Value::U64(u64::from(u32::MAX) + 1).try_as_u32(), None);
    assert_eq!(Value::Bytes(Arc::from([1, 2, 3, 4, 5])).try_as_u32(), None);
    assert_eq!(Value::Float(-1.0).try_as_u32(), None);
    assert_eq!(Value::Float(f64::NAN).try_as_u32(), None);
    assert_eq!(Value::Float(f64::INFINITY).try_as_u32(), None);
    assert_eq!(Value::Float(f64::from(u32::MAX) + 1.0).try_as_u32(), None);

    assert_eq!(Value::I32(-1).try_as_u64(), None);
    assert_eq!(
        Value::Bytes(Arc::from([1, 2, 3, 4, 5, 6, 7, 8, 9])).try_as_u64(),
        None
    );
    assert_eq!(Value::Float(-1.0).try_as_u64(), None);
    assert_eq!(Value::Float(f64::NAN).try_as_u64(), None);
    assert_eq!(Value::Float(f64::NEG_INFINITY).try_as_u64(), None);
    assert_eq!(
        Value::Float(18_446_744_073_709_551_616.0).try_as_u64(),
        None
    );
}

#[test]
fn finite_float_narrowing_truncates_only_inside_unsigned_range() {
    assert_eq!(Value::Float(0.0).try_as_u32(), Some(0));
    assert_eq!(Value::Float(42.9).try_as_u32(), Some(42));
    assert_eq!(
        Value::Float(f64::from(u32::MAX)).try_as_u32(),
        Some(u32::MAX)
    );

    assert_eq!(Value::Float(0.0).try_as_u64(), Some(0));
    assert_eq!(Value::Float(42.9).try_as_u64(), Some(42));
    assert_eq!(
        Value::Float(9_007_199_254_740_992.0).try_as_u64(),
        Some(9_007_199_254_740_992)
    );
}

#[test]
fn short_byte_scalars_zero_pad_as_little_endian_prefixes() {
    assert_eq!(Value::Bytes(Arc::from([])).try_as_u32(), Some(0));
    assert_eq!(Value::Bytes(Arc::from([0xAA])).try_as_u32(), Some(0xAA));
    assert_eq!(
        Value::Bytes(Arc::from([0xAA, 0xBB, 0xCC])).try_as_u32(),
        Some(0x00CC_BBAA)
    );
    assert_eq!(
        Value::Bytes(Arc::from([1, 2, 3, 4, 5])).try_as_u64(),
        Some(0x0000_0005_0403_0201)
    );
}

#[test]
fn truthiness_matches_ir_word_convention() {
    assert!(!Value::U32(0).truthy());
    assert!(Value::U32(1).truthy());
    assert!(!Value::Bool(false).truthy());
    assert!(Value::Bool(true).truthy());
    assert!(!Value::Bytes(Arc::from([])).truthy());
    assert!(Value::Bytes(Arc::from([0, 0, 0, 0, 1])).truthy());
    assert!(!Value::Array(Vec::new()).truthy());
    assert!(Value::Array(vec![Value::U32(0)]).truthy());
    assert!(Value::Float(f64::NAN).truthy());
    assert!(!Value::Float(0.0).truthy());
}

#[test]
fn float_equality_is_bit_exact_not_numeric_approximation() {
    assert_ne!(Value::Float(0.0), Value::Float(-0.0));
    assert_eq!(
        Value::Float(f64::from_bits(0x7FF8_0000_0000_0001)),
        Value::Float(f64::from_bits(0x7FF8_0000_0000_0001))
    );
    assert_ne!(
        Value::Float(f64::from_bits(0x7FF8_0000_0000_0001)),
        Value::Float(f64::from_bits(0x7FF8_0000_0000_0002))
    );
}

#[test]
fn from_element_bytes_rejects_short_fixed_width_inputs() {
    assert!(matches!(
        Value::from_element_bytes(DataType::U32, &[1, 2, 3]),
        Err(_)
    ));
    assert!(matches!(
        Value::from_element_bytes(DataType::U64, &[1, 2, 3, 4, 5, 6, 7]),
        Err(_)
    ));
    assert!(matches!(
        Value::from_element_bytes(DataType::Bool, &[1, 0, 0]),
        Err(_)
    ));
    assert!(Value::from_element_bytes(DataType::Vec2U32, &[0; 7].as_slice()).is_err());
    assert!(Value::from_element_bytes(DataType::Vec4U32, &[0; 15].as_slice()).is_err());
    assert!(Value::from_element_bytes(DataType::F32, &[0; 3].as_slice()).is_err());
    assert!(Value::from_element_bytes(DataType::F64, &[0; 7].as_slice()).is_err());
    assert!(Value::from_element_bytes(DataType::F16, &[0; 1].as_slice()).is_err());
    assert!(Value::from_element_bytes(DataType::I16, &[0; 1].as_slice()).is_err());
}

#[test]
fn zero_for_returns_exact_public_zero_shapes() {
    assert_eq!(Value::zero_for(DataType::U32), Value::U32(0));
    assert_eq!(Value::zero_for(DataType::I32), Value::I32(0));
    assert_eq!(Value::zero_for(DataType::U64), Value::U64(0));
    assert_eq!(Value::zero_for(DataType::Bool), Value::Bool(false));
    assert_eq!(Value::zero_for(DataType::F32), Value::Float(0.0));
    assert_eq!(Value::zero_for(DataType::F64), Value::Float(0.0));
    assert_eq!(Value::zero_for(DataType::Vec2U32).to_bytes(), vec![0; 8]);
    assert_eq!(Value::zero_for(DataType::Vec4U32).to_bytes(), vec![0; 16]);
}

#[test]
fn quantized_and_extended_fixed_width_values_have_typed_storage_widths() {
    let cases = [
        (DataType::U8, vec![0xAB], vec![0]),
        (DataType::I8, vec![0x80], vec![0]),
        (DataType::U16, vec![0x34, 0x12], vec![0, 0]),
        (DataType::I16, vec![0xFE, 0xFF], vec![0, 0]),
        (DataType::F16, vec![0x00, 0x3C], vec![0, 0]),
        (DataType::BF16, vec![0x80, 0x3F], vec![0, 0]),
        (DataType::F8E4M3, vec![0x7F], vec![0]),
        (DataType::F8E5M2, vec![0x7B], vec![0]),
        (DataType::I4, vec![0x0F], vec![0]),
        (DataType::FP4, vec![0x06], vec![0]),
        (DataType::NF4, vec![0x08], vec![0]),
        (DataType::I64, vec![1, 2, 3, 4, 5, 6, 7, 8], vec![0; 8]),
    ];

    for (ty, encoded, zero) in cases {
        let value = Value::from_element_bytes(ty.clone(), &encoded)
            .expect("fixed-width type must decode from its exact storage width");
        assert_eq!(
            value.to_bytes(),
            encoded,
            "{ty} must preserve raw storage bits"
        );
        assert_eq!(
            Value::zero_for(ty.clone()).to_bytes(),
            zero,
            "{ty} must have a typed zero payload, not an empty Bytes fallback"
        );
    }
}

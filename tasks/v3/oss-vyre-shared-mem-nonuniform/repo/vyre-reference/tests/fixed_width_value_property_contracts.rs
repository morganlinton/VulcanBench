//! Generated property coverage for fixed-width `Value::from_element_bytes`.

use proptest::prelude::*;
use vyre::ir::DataType;
use vyre_reference::ieee754::canonical_f32;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_u32_element_bytes_decode_little_endian(value in any::<u32>()) {
        let bytes = value.to_le_bytes();
        prop_assert_eq!(
            Value::from_element_bytes(DataType::U32, &bytes)
                .expect("Fix: exact u32 bytes must decode"),
            Value::U32(value)
        );
    }

    #[test]
    fn generated_i32_element_bytes_decode_little_endian(value in any::<i32>()) {
        let bytes = value.to_le_bytes();
        prop_assert_eq!(
            Value::from_element_bytes(DataType::I32, &bytes)
                .expect("Fix: exact i32 bytes must decode"),
            Value::I32(value)
        );
    }

    #[test]
    fn generated_u64_element_bytes_decode_little_endian(value in any::<u64>()) {
        let bytes = value.to_le_bytes();
        prop_assert_eq!(
            Value::from_element_bytes(DataType::U64, &bytes)
                .expect("Fix: exact u64 bytes must decode"),
            Value::U64(value)
        );
    }

    #[test]
    fn generated_bool_element_bytes_use_ir_word_convention(word in any::<u32>()) {
        let bytes = word.to_le_bytes();
        prop_assert_eq!(
            Value::from_element_bytes(DataType::Bool, &bytes)
                .expect("Fix: exact bool word bytes must decode"),
            Value::Bool(word != 0)
        );
    }

    #[test]
    fn generated_f32_element_bytes_follow_reference_canonicalization(bits in any::<u32>()) {
        let value = Value::from_element_bytes(DataType::F32, &bits.to_le_bytes())
            .expect("Fix: exact f32 bytes must decode");
        let expected = f64::from(canonical_f32(f32::from_bits(bits))).to_le_bytes();
        prop_assert_eq!(value.to_bytes(), expected);
    }
}

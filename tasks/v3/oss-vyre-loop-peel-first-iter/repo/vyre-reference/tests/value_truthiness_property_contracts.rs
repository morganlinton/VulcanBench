//! Generated property coverage for IR truthiness conventions.

use std::sync::Arc;

use proptest::prelude::*;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_u32_truthiness_is_nonzero(value in any::<u32>()) {
        prop_assert_eq!(Value::U32(value).truthy(), value != 0);
    }

    #[test]
    fn generated_i32_truthiness_uses_lossless_unsigned_conversion(value in any::<i32>()) {
        prop_assert_eq!(Value::I32(value).truthy(), u32::try_from(value).unwrap_or(1) != 0);
    }

    #[test]
    fn generated_bool_truthiness_is_boolean_value(value in any::<bool>()) {
        prop_assert_eq!(Value::Bool(value).truthy(), value);
    }

    #[test]
    fn generated_bytes_truthiness_uses_u32_only_for_short_scalars(bytes in prop::collection::vec(any::<u8>(), 0..=16)) {
        let expected = if bytes.len() <= 4 {
            let mut padded = [0u8; 4];
            padded[..bytes.len()].copy_from_slice(&bytes);
            u32::from_le_bytes(padded) != 0
        } else {
            true
        };
        prop_assert_eq!(Value::Bytes(Arc::from(bytes)).truthy(), expected);
    }

    #[test]
    fn generated_array_truthiness_is_non_empty(len in 0usize..=32usize) {
        let values = vec![Value::U32(0); len];
        prop_assert_eq!(Value::Array(values).truthy(), len != 0);
    }
}

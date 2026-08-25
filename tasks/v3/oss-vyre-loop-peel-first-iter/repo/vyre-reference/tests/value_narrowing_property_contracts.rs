//! Generated property coverage for scalar narrowing in the reference oracle.

use proptest::prelude::*;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_i32_to_u32_succeeds_only_for_non_negative_values(value in any::<i32>()) {
        prop_assert_eq!(Value::I32(value).try_as_u32(), u32::try_from(value).ok());
    }

    #[test]
    fn generated_i32_to_u64_succeeds_only_for_non_negative_values(value in any::<i32>()) {
        prop_assert_eq!(Value::I32(value).try_as_u64(), u64::try_from(value).ok());
    }

    #[test]
    fn generated_u64_to_u32_succeeds_only_inside_u32_range(value in any::<u64>()) {
        prop_assert_eq!(Value::U64(value).try_as_u32(), u32::try_from(value).ok());
    }

    #[test]
    fn generated_finite_float_to_u32_matches_truncation_inside_range(value in 0.0f64..=u32::MAX as f64) {
        prop_assert_eq!(Value::Float(value).try_as_u32(), Some(value as u32));
    }

    #[test]
    fn generated_negative_float_to_unsigned_is_rejected(value in -1.0e12f64..0.0f64) {
        prop_assert_eq!(Value::Float(value).try_as_u32(), None);
        prop_assert_eq!(Value::Float(value).try_as_u64(), None);
    }
}

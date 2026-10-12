//! Property contracts for signed integer narrowing and byte encoding.

use proptest::prelude::*;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_i32_narrowing_matches_checked_unsigned_conversions(raw in any::<i32>()) {
        let value = Value::I32(raw);

        prop_assert_eq!(value.try_as_u32(), u32::try_from(raw).ok());
        prop_assert_eq!(value.try_as_u64(), u64::try_from(raw).ok());
        prop_assert_eq!(value.as_u32(), u32::try_from(raw).unwrap_or(0));
        prop_assert_eq!(value.as_u64(), u64::try_from(raw).unwrap_or(0));
    }

    #[test]
    fn generated_i32_bytes_are_little_endian(raw in any::<i32>()) {
        let value = Value::I32(raw);

        prop_assert_eq!(value.to_bytes(), raw.to_le_bytes().to_vec());
        prop_assert_eq!(value.wide_bytes(), raw.to_le_bytes().to_vec());
    }

    #[test]
    fn generated_i32_width_encoding_is_prefix_then_zero_pad(raw in any::<i32>(), width in 0usize..12) {
        let value = Value::I32(raw);
        let mut expected = raw.to_le_bytes().to_vec();

        if width != 0 {
            expected.resize(width, 0);
            expected.truncate(width);
        }

        prop_assert_eq!(value.to_bytes_width(width), expected);
    }
}

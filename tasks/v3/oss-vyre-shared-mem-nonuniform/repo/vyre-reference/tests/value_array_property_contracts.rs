//! Property contracts for nested value-array byte encoding.

use proptest::prelude::*;
use vyre_reference::value::Value;

fn u32_array(values: &[u32]) -> Value {
    Value::Array(values.iter().copied().map(Value::U32).collect())
}

fn flattened_u32_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

proptest! {
    #[test]
    fn generated_u32_arrays_flatten_in_element_order(values in prop::collection::vec(any::<u32>(), 0..64)) {
        let value = u32_array(&values);

        prop_assert_eq!(value.to_bytes(), flattened_u32_bytes(&values));
        prop_assert_eq!(value.truthy(), !values.is_empty());
        prop_assert_eq!(value.try_as_u32(), None);
        prop_assert_eq!(value.try_as_u64(), None);
    }

    #[test]
    fn generated_u32_array_width_encoding_matches_extend_api(
        prefix in prop::collection::vec(any::<u8>(), 0..32),
        values in prop::collection::vec(any::<u32>(), 0..64),
        width in 0usize..96,
    ) {
        let value = u32_array(&values);
        let mut encoded = prefix.clone();
        value
            .extend_bytes_width(width, &mut encoded)
            .expect("generated array byte extension must not overflow for bounded inputs");

        let mut expected = prefix;
        expected.extend(value.to_bytes_width(width));

        prop_assert_eq!(encoded, expected);
    }
}

//! Generated property coverage for reference `Value` byte encoding.

use std::sync::Arc;

use proptest::prelude::*;
use vyre_reference::value::Value;

fn byte_value_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<u32>().prop_map(Value::U32),
        any::<i32>().prop_map(Value::I32),
        any::<u64>().prop_map(Value::U64),
        any::<bool>().prop_map(Value::Bool),
        any::<u64>().prop_map(|bits| Value::Float(f64::from_bits(bits))),
        prop::collection::vec(any::<u8>(), 0..=64).prop_map(|bytes| Value::Bytes(Arc::from(bytes))),
        prop::collection::vec(any::<u32>().prop_map(Value::U32), 0..=16).prop_map(Value::Array),
    ]
}

proptest! {
    #[test]
    fn generated_extend_bytes_width_matches_allocating_encoding(value in byte_value_strategy(), width in 0usize..=64usize) {
        let mut extended = Vec::new();
        value.extend_bytes_width(width, &mut extended)
            .expect("Fix: bounded generated values must extend without size overflow");

        if width != 0 {
            prop_assert_eq!(extended.len(), width);
        }
        prop_assert_eq!(extended, value.to_bytes_width(width));
    }

    #[test]
    fn generated_to_bytes_width_has_exact_declared_width_when_nonzero(value in byte_value_strategy(), width in 1usize..=64usize) {
        prop_assert_eq!(value.to_bytes_width(width).len(), width);
    }

    #[test]
    fn generated_zero_width_encoding_is_raw_payload(value in byte_value_strategy()) {
        prop_assert_eq!(value.to_bytes_width(0), value.to_bytes());
    }
}

//! Generated property coverage for little-endian byte-prefix scalar decoding.

use std::sync::Arc;

use proptest::prelude::*;
use vyre_reference::value::Value;

proptest! {
    #[test]
    fn generated_short_byte_prefix_decodes_as_zero_padded_u32(bytes in prop::collection::vec(any::<u8>(), 0..=4)) {
        let mut padded = [0u8; 4];
        padded[..bytes.len()].copy_from_slice(&bytes);
        prop_assert_eq!(
            Value::Bytes(Arc::from(bytes)).try_as_u32(),
            Some(u32::from_le_bytes(padded))
        );
    }

    #[test]
    fn generated_longer_than_u32_byte_prefix_rejects_u32(bytes in prop::collection::vec(any::<u8>(), 5..=32)) {
        prop_assert_eq!(Value::Bytes(Arc::from(bytes)).try_as_u32(), None);
    }

    #[test]
    fn generated_short_byte_prefix_decodes_as_zero_padded_u64(bytes in prop::collection::vec(any::<u8>(), 0..=8)) {
        let mut padded = [0u8; 8];
        padded[..bytes.len()].copy_from_slice(&bytes);
        prop_assert_eq!(
            Value::Bytes(Arc::from(bytes)).try_as_u64(),
            Some(u64::from_le_bytes(padded))
        );
    }

    #[test]
    fn generated_longer_than_u64_byte_prefix_rejects_u64(bytes in prop::collection::vec(any::<u8>(), 9..=32)) {
        prop_assert_eq!(Value::Bytes(Arc::from(bytes)).try_as_u64(), None);
    }
}

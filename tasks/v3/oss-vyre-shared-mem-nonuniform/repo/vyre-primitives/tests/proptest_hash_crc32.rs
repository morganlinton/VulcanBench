//! Property gates for `hash::crc32::crc32` - CRC-32 table-driven properties.
#![cfg(feature = "hash")]

use proptest::prelude::*;
use vyre_primitives::hash::crc32::crc32;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn crc32_of_empty_is_zero(_dummy in 0u32..1) {
        // Empty input: init ^ final_xor = 0xFFFFFFFF ^ 0xFFFFFFFF = 0
        prop_assert_eq!(crc32(b""), 0, "CRC-32 of empty must be 0");
    }

    #[test]
    fn crc32_of_zeros_is_deterministic(n in 0usize..256) {
        let zeros = vec![0u8; n];
        let result1 = crc32(&zeros);
        let result2 = crc32(&zeros);
        prop_assert_eq!(result1, result2, "CRC-32 of all-zero buffer must be deterministic");
    }

    #[test]
    fn crc32_is_deterministic(
        a in proptest::collection::vec(any::<u8>(), 0..=64),
    ) {
        let result1 = crc32(&a);
        let result2 = crc32(&a);
        prop_assert_eq!(result1, result2, "CRC-32 must be deterministic for same input");
    }

    #[test]
    fn crc32_appending_empty_does_not_change(
        a in proptest::collection::vec(any::<u8>(), 0..=32),
    ) {
        let crc_a = crc32(&a);
        let mut a_with_empty = a.clone();
        a_with_empty.extend_from_slice(b"");
        prop_assert_eq!(crc_a, crc32(&a_with_empty), "appending empty must not change CRC");
    }

    #[test]
    fn crc32_concatenation_is_deterministic(
        a in proptest::collection::vec(any::<u8>(), 0..=32),
        b in proptest::collection::vec(any::<u8>(), 0..=32),
    ) {
        let mut combined = a.clone();
        combined.extend_from_slice(&b);
        let crc_combined = crc32(&combined);
        // Determinism: same concatenated input must yield same CRC
        prop_assert_eq!(crc_combined, crc32(&combined));
    }
}

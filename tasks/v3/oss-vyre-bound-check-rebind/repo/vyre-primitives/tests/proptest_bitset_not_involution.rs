//! Property gates for `bitset::not::cpu_ref` - bitwise NOT involution.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::equal::cpu_ref as bitset_equal;
use vyre_primitives::bitset::not::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn not_is_involution(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let not_a = cpu_ref(&a);
        let not_not_a = cpu_ref(&not_a);
        prop_assert_eq!(bitset_equal(&a, &not_not_a), 1, "!!a must equal a");
    }

    #[test]
    fn not_all_zeros_is_all_ones(n in 0usize..64) {
        let zeros = vec![0u32; n];
        let ones = vec![0xFFFFFFFFu32; n];
        let result = cpu_ref(&zeros);
        prop_assert_eq!(bitset_equal(&result, &ones), 1, "!0 must equal 1");
    }

    #[test]
    fn not_all_ones_is_all_zeros(n in 0usize..64) {
        let ones = vec![0xFFFFFFFFu32; n];
        let zeros = vec![0u32; n];
        let result = cpu_ref(&ones);
        prop_assert_eq!(bitset_equal(&result, &zeros), 1, "!1 must equal 0");
    }

    #[test]
    fn not_preserves_length(
        a in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        let result = cpu_ref(&a);
        prop_assert_eq!(result.len(), a.len(), "not must preserve length");
    }
}

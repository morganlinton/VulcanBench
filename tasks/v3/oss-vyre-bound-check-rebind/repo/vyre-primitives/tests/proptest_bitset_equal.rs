//! Property gates for `bitset::equal::cpu_ref` - bitset equality predicate.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::equal::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn equal_reflexive(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        prop_assert_eq!(cpu_ref(&a, &a), 1, "a == a must be true");
    }

    #[test]
    fn equal_symmetric(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
        b in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let ab = cpu_ref(&a, &b);
        let ba = cpu_ref(&b, &a);
        prop_assert_eq!(ab, ba, "equality must be symmetric");
    }

    #[test]
    fn equal_with_zeros_checks_all_zero(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let zeros = vec![0u32; a.len()];
        let result = cpu_ref(&a, &zeros);
        let all_zero = a.iter().all(|&w| w == 0);
        prop_assert_eq!(result, if all_zero { 1 } else { 0 });
    }

    #[test]
    fn equal_with_ones_checks_all_ones(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let ones = vec![0xFFFFFFFFu32; a.len()];
        let result = cpu_ref(&a, &ones);
        let all_ones = a.iter().all(|&w| w == 0xFFFFFFFF);
        prop_assert_eq!(result, if all_ones { 1 } else { 0 });
    }

    #[test]
    fn empty_bitsets_are_equal(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[], &[]), 1);
    }
}

//! Property gates for `vyre_primitives::bitset::popcount::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::popcount::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn cpu_ref_matches_std_count_ones(
        input in proptest::collection::vec(any::<u32>(), 0..=32),
    ) {
        let expected: Vec<u32> = input.iter().map(|w| w.count_ones()).collect();
        prop_assert_eq!(cpu_ref(&input), expected);
    }

    #[test]
    fn popcount_of_zeros_is_zero(
        len in 0usize..16,
    ) {
        let input = vec![0u32; len];
        let expected = vec![0u32; len];
        prop_assert_eq!(cpu_ref(&input), expected);
    }

    #[test]
    fn popcount_of_ones_is_32(
        len in 1usize..16,
    ) {
        let input = vec![u32::MAX; len];
        let expected = vec![32u32; len];
        prop_assert_eq!(cpu_ref(&input), expected);
    }

    #[test]
    fn popcount_is_additive_over_concat(
        a in proptest::collection::vec(any::<u32>(), 0..=8),
        b in proptest::collection::vec(any::<u32>(), 0..=8),
    ) {
        let mut concat = a.clone();
        concat.extend_from_slice(&b);
        let result = cpu_ref(&concat);
        let expected_a = cpu_ref(&a);
        let expected_b = cpu_ref(&b);
        let mut expected = expected_a;
        expected.extend_from_slice(&expected_b);
        prop_assert_eq!(result, expected);
    }
}

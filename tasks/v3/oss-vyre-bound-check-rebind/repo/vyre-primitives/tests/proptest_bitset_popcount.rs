//! Property gates for `bitset::popcount::cpu_ref` - per-word population count.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::popcount::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn popcount_of_empty_is_empty(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[]), Vec::<u32>::new());
    }

    #[test]
    fn popcount_preserves_length(
        words in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        let result = cpu_ref(&words);
        prop_assert_eq!(result.len(), words.len(), "popcount must preserve length");
    }

    #[test]
    fn popcount_of_zeros_is_zeros(n in 0usize..64) {
        let zeros = vec![0u32; n];
        let result = cpu_ref(&zeros);
        prop_assert_eq!(result, vec![0u32; n]);
    }

    #[test]
    fn popcount_of_all_ones_is_32(n in 0usize..64) {
        let ones = vec![0xFFFFFFFFu32; n];
        let result = cpu_ref(&ones);
        prop_assert_eq!(result, vec![32u32; n]);
    }

    #[test]
    fn popcount_matches_count_ones(
        words in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        let result = cpu_ref(&words);
        for (i, &w) in words.iter().enumerate() {
            prop_assert_eq!(result[i], w.count_ones(), "word {} popcount mismatch", i);
        }
    }

    #[test]
    fn popcount_is_additive_over_concatenation(
        a in proptest::collection::vec(any::<u32>(), 0..=32),
        b in proptest::collection::vec(any::<u32>(), 0..=32),
    ) {
        let mut combined = a.clone();
        combined.extend_from_slice(&b);
        let count_a = cpu_ref(&a);
        let count_b = cpu_ref(&b);
        let count_combined = cpu_ref(&combined);
        let mut expected = count_a;
        expected.extend_from_slice(&count_b);
        prop_assert_eq!(count_combined, expected, "popcount must be additive over concatenation");
    }
}

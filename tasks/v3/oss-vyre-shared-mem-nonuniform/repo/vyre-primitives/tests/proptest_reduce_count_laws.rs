//! Property gates for `reduce::count::cpu_ref` - population count monoid.
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::count::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn count_of_empty_is_zero(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[]), 0);
    }

    #[test]
    fn count_of_all_zeros_is_zero(n in 0usize..64) {
        prop_assert_eq!(cpu_ref(&vec![0u32; n]), 0);
    }

    #[test]
    fn count_of_all_ones_is_32_times_len(n in 0usize..64) {
        let ones = vec![0xFFFFFFFFu32; n];
        prop_assert_eq!(cpu_ref(&ones), (n as u32) * 32);
    }

    #[test]
    fn count_is_additive_over_concatenation(
        a in proptest::collection::vec(any::<u32>(), 0..=32),
        b in proptest::collection::vec(any::<u32>(), 0..=32),
    ) {
        let mut combined = a.clone();
        combined.extend_from_slice(&b);
        let count_a = cpu_ref(&a);
        let count_b = cpu_ref(&b);
        let count_combined = cpu_ref(&combined);
        prop_assert_eq!(
            count_combined,
            count_a.wrapping_add(count_b),
            "count(a ++ b) must equal count(a) + count(b)"
        );
    }

    #[test]
    fn count_matches_manual_popcount(
        words in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        let expected: u32 = words.iter().map(|w| w.count_ones()).sum();
        prop_assert_eq!(cpu_ref(&words), expected);
    }

    #[test]
    fn count_of_single_word_matches_count_ones(w in any::<u32>()) {
        prop_assert_eq!(cpu_ref(&[w]), w.count_ones());
    }
}

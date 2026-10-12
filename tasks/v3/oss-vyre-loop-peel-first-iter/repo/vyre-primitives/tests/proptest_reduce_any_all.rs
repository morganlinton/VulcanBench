//! Property gates for `reduce::any::cpu_ref` and `reduce::all::cpu_ref`.
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::all::cpu_ref as all_ref;
use vyre_primitives::reduce::any::cpu_ref as any_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn any_of_empty_is_zero(_dummy in 0u32..1) {
        prop_assert_eq!(any_ref(&[]), 0);
    }

    #[test]
    fn all_of_empty_is_one(_dummy in 0u32..1) {
        prop_assert_eq!(all_ref(&[]), 1);
    }

    #[test]
    fn any_of_all_zeros_is_zero(n in 0usize..64) {
        prop_assert_eq!(any_ref(&vec![0u32; n]), 0);
    }

    #[test]
    fn all_of_all_zeros_is_zero(n in 0usize..64) {
        prop_assert_eq!(all_ref(&vec![0u32; n]), if n == 0 { 1 } else { 0 });
    }

    #[test]
    fn any_of_all_ones_is_one(n in 0usize..64) {
        prop_assert_eq!(any_ref(&vec![0xFFFFFFFFu32; n]), if n == 0 { 0 } else { 1 });
    }

    #[test]
    fn all_of_all_ones_is_one(n in 0usize..64) {
        prop_assert_eq!(all_ref(&vec![0xFFFFFFFFu32; n]), 1);
    }

    #[test]
    fn any_single_word_matches_truthiness(w in any::<u32>()) {
        prop_assert_eq!(any_ref(&[w]), if w != 0 { 1 } else { 0 });
    }

    #[test]
    fn all_single_word_matches_truthiness(w in any::<u32>()) {
        prop_assert_eq!(all_ref(&[w]), if w != 0 { 1 } else { 0 });
    }

    #[test]
    fn any_and_all_are_dual_for_single_word(w in any::<u32>()) {
        prop_assert!(
            any_ref(&[w]) == 1 || all_ref(&[w]) == 0,
            "any=0 implies all=0 for single word"
        );
        prop_assert!(
            all_ref(&[w]) == 1 || any_ref(&[w]) == 1,
            "all=1 implies any=1 for single word"
        );
    }
}

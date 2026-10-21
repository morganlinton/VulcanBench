//! Property gates for `bitset::subset_of::cpu_ref` - subset predicate laws.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::subset_of::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn empty_is_subset_of_anything(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let empty: Vec<u32> = vec![0u32; a.len()];
        prop_assert_eq!(cpu_ref(&empty, &a), 1, "empty set must be subset of any set");
    }

    #[test]
    fn anything_is_subset_of_all_ones(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let ones = vec![0xFFFFFFFFu32; a.len()];
        prop_assert_eq!(cpu_ref(&a, &ones), 1, "any set must be subset of universal set");
    }

    #[test]
    fn subset_of_self_is_always_true(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        prop_assert_eq!(cpu_ref(&a, &a), 1, "a must be subset of itself");
    }

    #[test]
    fn subset_of_zeros_implies_all_zeros(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let zeros = vec![0u32; a.len()];
        let result = cpu_ref(&a, &zeros);
        let all_zero = a.iter().all(|&w| w == 0);
        prop_assert_eq!(result, if all_zero { 1 } else { 0 });
    }

    #[test]
    fn subset_is_transitive(
        a in proptest::collection::vec(any::<u32>(), 0..=8),
        b in proptest::collection::vec(any::<u32>(), 0..=8),
        c in proptest::collection::vec(any::<u32>(), 0..=8),
    ) {
        let ab = cpu_ref(&a, &b);
        let bc = cpu_ref(&b, &c);
        if ab == 1 && bc == 1 {
            let ac = cpu_ref(&a, &c);
            prop_assert_eq!(ac, 1, "subset relation must be transitive");
        }
    }

    #[test]
    fn and_result_is_subset_of_lhs(
        a in proptest::collection::vec(any::<u32>(), 0..=8),
        b in proptest::collection::vec(any::<u32>(), 0..=8),
    ) {
        let and_ab = vyre_primitives::bitset::and::cpu_ref(&a, &b);
        prop_assert_eq!(cpu_ref(&and_ab, &a), 1, "a & b must be subset of a");
        prop_assert_eq!(cpu_ref(&and_ab, &b), 1, "a & b must be subset of b");
    }
}

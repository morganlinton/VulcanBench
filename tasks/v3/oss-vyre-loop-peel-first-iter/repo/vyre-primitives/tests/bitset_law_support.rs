//! Shared property-test helpers for bitset law integration tests.

#![allow(unused_macros)]

pub(crate) fn manual_bitset_binary(
    lhs: &[u32],
    rhs: &[u32],
    op: impl Fn(u32, u32) -> u32,
) -> Vec<u32> {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(a, b)| op(*a, *b))
        .collect()
}

macro_rules! bitset_and_law_tests {
    ($cpu_ref:path) => {
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(10_000))]

            #[test]
            fn cpu_ref_matches_manual_and(
                lhs in proptest::collection::vec(any::<u32>(), 0..=32),
                rhs in proptest::collection::vec(any::<u32>(), 0..=32),
            ) {
                prop_assert_eq!(
                    $cpu_ref(&lhs, &rhs),
                    crate::bitset_law_support::manual_bitset_binary(&lhs, &rhs, |a, b| a & b)
                );
            }

            #[test]
            fn and_is_commutative(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
                rhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let n = lhs.len().min(rhs.len());
                let l = &lhs[..n];
                let r = &rhs[..n];
                prop_assert_eq!($cpu_ref(l, r), $cpu_ref(r, l));
            }

            #[test]
            fn and_with_zero_rhs_is_zero(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let zero = vec![0u32; lhs.len()];
                let out = $cpu_ref(&lhs, &zero);
                prop_assert!(out.iter().all(|w| *w == 0));
            }

            #[test]
            fn and_with_ones_rhs_preserves_lhs(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let ones = vec![u32::MAX; lhs.len()];
                prop_assert_eq!($cpu_ref(&lhs, &ones), lhs);
            }

            #[test]
            fn and_is_idempotent(
                v in proptest::collection::vec(any::<u32>(), 0..=16),
            ) {
                prop_assert_eq!($cpu_ref(&v, &v), v);
            }
        }
    };
}

macro_rules! bitset_or_law_tests {
    ($cpu_ref:path) => {
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(10_000))]

            #[test]
            fn cpu_ref_matches_manual_or(
                lhs in proptest::collection::vec(any::<u32>(), 0..=32),
                rhs in proptest::collection::vec(any::<u32>(), 0..=32),
            ) {
                prop_assert_eq!(
                    $cpu_ref(&lhs, &rhs),
                    crate::bitset_law_support::manual_bitset_binary(&lhs, &rhs, |a, b| a | b)
                );
            }

            #[test]
            fn or_is_commutative(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
                rhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let n = lhs.len().min(rhs.len());
                let l = &lhs[..n];
                let r = &rhs[..n];
                prop_assert_eq!($cpu_ref(l, r), $cpu_ref(r, l));
            }

            #[test]
            fn or_with_zero_rhs_is_identity(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let zero = vec![0u32; lhs.len()];
                prop_assert_eq!($cpu_ref(&lhs, &zero), lhs);
            }

            #[test]
            fn or_with_ones_rhs_is_all_ones(
                lhs in proptest::collection::vec(any::<u32>(), 1..=16),
            ) {
                let ones = vec![u32::MAX; lhs.len()];
                prop_assert_eq!($cpu_ref(&lhs, &ones), ones);
            }

            #[test]
            fn or_is_idempotent(
                v in proptest::collection::vec(any::<u32>(), 0..=16),
            ) {
                prop_assert_eq!($cpu_ref(&v, &v), v);
            }
        }
    };
}

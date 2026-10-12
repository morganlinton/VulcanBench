//! Property gates for `vyre_primitives::reduce::sum::cpu_ref`.

#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::sum::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn cpu_ref_matches_wrapping_fold(values in proptest::collection::vec(any::<u32>(), 0..=64)) {
        let expected = values.iter().copied().fold(0u32, u32::wrapping_add);
        prop_assert_eq!(cpu_ref(&values), expected);
    }

    #[test]
    fn sum_of_empty_is_zero(_dummy in 0..1) {
        let _ = _dummy;
        prop_assert_eq!(cpu_ref(&[]), 0u32);
    }

    #[test]
    fn sum_of_zeros_is_zero(len in 0usize..64) {
        let values = vec![0u32; len];
        prop_assert_eq!(cpu_ref(&values), 0u32);
    }

    #[test]
    fn sum_is_associative_for_pairs(a in any::<u32>(), b in any::<u32>(), c in any::<u32>(), d in any::<u32>()) {
        let left = cpu_ref(&[a, b]);
        let right = cpu_ref(&[c, d]);
        let total = cpu_ref(&[a, b, c, d]);
        prop_assert_eq!(u32::wrapping_add(left, right), total);
    }
}

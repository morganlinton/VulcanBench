//! Property gates for `vyre_primitives::reduce::min::cpu_ref` and
//! `vyre_primitives::reduce::max::cpu_ref`.

#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::max::cpu_ref as max_ref;
use vyre_primitives::reduce::min::cpu_ref as min_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn min_ref_matches_std_min(values in proptest::collection::vec(any::<u32>(), 0..=64)) {
        let expected = values.iter().copied().min().unwrap_or(u32::MAX);
        prop_assert_eq!(min_ref(&values), expected);
    }

    #[test]
    fn max_ref_matches_std_max(values in proptest::collection::vec(any::<u32>(), 0..=64)) {
        let expected = values.iter().copied().max().unwrap_or(0);
        prop_assert_eq!(max_ref(&values), expected);
    }

    #[test]
    fn min_of_singleton_is_itself(v in any::<u32>()) {
        prop_assert_eq!(min_ref(&[v]), v);
    }

    #[test]
    fn max_of_singleton_is_itself(v in any::<u32>()) {
        prop_assert_eq!(max_ref(&[v]), v);
    }

    #[test]
    fn min_le_max_for_any_slice(values in proptest::collection::vec(any::<u32>(), 1..=64)) {
        prop_assert!(min_ref(&values) <= max_ref(&values));
    }

    #[test]
    fn min_of_empty_is_u32_max(_dummy in 0..1) {
        let _ = _dummy;
        prop_assert_eq!(min_ref(&[]), u32::MAX);
    }

    #[test]
    fn max_of_empty_is_zero(_dummy in 0..1) {
        let _ = _dummy;
        prop_assert_eq!(max_ref(&[]), 0u32);
    }
}

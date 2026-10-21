//! Property gates for `vyre_primitives::reduce::all::cpu_ref`.

#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::all::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn all_nonzero_returns_one(
        values in proptest::collection::vec(any::<u32>(), 1..=64),
    ) {
        let values: Vec<u32> = values.iter().map(|v| if *v == 0 { 1 } else { *v }).collect();
        prop_assert_eq!(cpu_ref(&values), 1);
    }

    #[test]
    fn one_zero_returns_zero(
        values in proptest::collection::vec(any::<u32>(), 1..=64),
        idx in any::<usize>(),
    ) {
        let mut values = values;
        let i = idx % values.len();
        values[i] = 0;
        prop_assert_eq!(cpu_ref(&values), 0);
    }

    #[test]
    fn empty_returns_one(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[]), 1);
    }

    #[test]
    fn single_element(v in any::<u32>()) {
        prop_assert_eq!(cpu_ref(&[v]), if v != 0 { 1 } else { 0 });
    }
}

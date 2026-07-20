//! Property gates for `vyre_primitives::reduce::any::cpu_ref`.

#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::reduce::any::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn any_nonzero_returns_one(
        values in proptest::collection::vec(any::<u32>(), 0..=64),
        nonzero in any::<u32>(),
        idx in any::<usize>(),
    ) {
        let nonzero = if nonzero == 0 { 1 } else { nonzero };
        let mut values = values;
        if !values.is_empty() {
            let i = idx % values.len();
            values[i] = nonzero;
        } else {
            values.push(nonzero);
        }
        prop_assert_eq!(cpu_ref(&values), 1);
    }

    #[test]
    fn all_zero_returns_zero(len in 0usize..64) {
        let values = vec![0u32; len];
        prop_assert_eq!(cpu_ref(&values), 0);
    }

    #[test]
    fn empty_returns_zero(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[]), 0);
    }

    #[test]
    fn single_nonzero_returns_one(v in any::<u32>()) {
        let v = if v == 0 { 1 } else { v };
        prop_assert_eq!(cpu_ref(&[v]), 1);
    }
}

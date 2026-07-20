//! Property gates for `vyre_primitives::bitset::not::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::not::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn not_inverts_every_word(input in proptest::collection::vec(any::<u32>(), 0..=32)) {
        let out = cpu_ref(&input);
        prop_assert_eq!(out.len(), input.len());
        for (a, b) in input.iter().zip(out.iter()) {
            prop_assert_eq!(*b, !*a);
        }
    }

    #[test]
    fn double_not_is_identity(input in proptest::collection::vec(any::<u32>(), 0..=32)) {
        let once = cpu_ref(&input);
        let twice = cpu_ref(&once);
        prop_assert_eq!(twice, input);
    }

    #[test]
    fn not_of_zero_is_all_ones(len in 1usize..=32usize) {
        let zero = vec![0u32; len];
        let out = cpu_ref(&zero);
        prop_assert!(out.iter().all(|w| *w == u32::MAX));
    }

    #[test]
    fn not_preserves_length(input in proptest::collection::vec(any::<u32>(), 0..=32)) {
        prop_assert_eq!(cpu_ref(&input).len(), input.len());
    }
}

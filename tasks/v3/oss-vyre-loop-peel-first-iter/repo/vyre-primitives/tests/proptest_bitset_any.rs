//! Property gates for `vyre_primitives::bitset::any::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::any::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn cpu_ref_matches_manual_iter_any(
        words in proptest::collection::vec(any::<u32>(), 0..=64),
    ) {
        let expected = u32::from(words.iter().any(|w| *w != 0));
        prop_assert_eq!(cpu_ref(&words), expected);
    }

    #[test]
    fn empty_returns_zero(_dummy in 0u32..1) {
        prop_assert_eq!(cpu_ref(&[]), 0);
    }

    #[test]
    fn all_zeros_returns_zero(len in 0usize..64) {
        let words = vec![0u32; len];
        prop_assert_eq!(cpu_ref(&words), 0);
    }

    #[test]
    fn any_nonzero_returns_one(
        words in proptest::collection::vec(any::<u32>(), 0..=64),
        nonzero in any::<u32>(),
        idx in any::<usize>(),
    ) {
        let nonzero = if nonzero == 0 { 1 } else { nonzero };
        let mut words = words;
        if !words.is_empty() {
            let i = idx % words.len();
            words[i] = nonzero;
        } else {
            words.push(nonzero);
        }
        prop_assert_eq!(cpu_ref(&words), 1);
    }

    #[test]
    fn single_word_boundary(
        word_idx in 0usize..8,
        bit_idx in 0u32..32,
    ) {
        let mut words = vec![0u32; 8];
        words[word_idx] = 1u32 << bit_idx;
        prop_assert_eq!(cpu_ref(&words), 1);
    }
}

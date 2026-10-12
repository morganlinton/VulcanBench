//! Property gates for `vyre_primitives::bitset::contains::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::contains::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn in_bounds_indices_match_bit_extraction(
        words in proptest::collection::vec(any::<u32>(), 0..=16),
        index in any::<u32>(),
    ) {
        let max_bits = (words.len() as u32) * 32;
        if max_bits > 0 {
            let idx = index % max_bits;
            let w = (idx / 32) as usize;
            let b = idx % 32;
            let expected = (words[w] >> b) & 1;
            prop_assert_eq!(cpu_ref(&words, idx), expected);
        }
    }

    #[test]
    fn out_of_bounds_returns_zero(
        words in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let max_bits = (words.len() as u32) * 32;
        prop_assert_eq!(cpu_ref(&words, max_bits.saturating_add(1)), 0);
    }

    #[test]
    fn set_bit_then_contains_one(
        words in proptest::collection::vec(any::<u32>(), 1..=16),
        word_idx in any::<usize>(),
        bit_idx in 0u32..32,
    ) {
        let w = word_idx % words.len();
        let mut words = words;
        words[w] |= 1u32 << bit_idx;
        let index = ((w as u32) * 32) + bit_idx;
        prop_assert_eq!(cpu_ref(&words, index), 1);
    }

    #[test]
    fn clear_bit_then_contains_zero(
        words in proptest::collection::vec(any::<u32>(), 1..=16),
        word_idx in any::<usize>(),
        bit_idx in 0u32..32,
    ) {
        let w = word_idx % words.len();
        let mut words = words;
        words[w] &= !(1u32 << bit_idx);
        let index = ((w as u32) * 32) + bit_idx;
        prop_assert_eq!(cpu_ref(&words, index), 0);
    }
}

//! Property gates for `vyre_primitives::bitset::copy::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::copy::cpu_ref;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn copy_preserves_data(
        source in proptest::collection::vec(any::<u32>(), 0..=32),
    ) {
        let mut target = vec![0u32; source.len()];
        cpu_ref(&mut target, &source);
        prop_assert_eq!(target, source);
    }

    #[test]
    fn copy_into_larger_target_preserves_excess(
        source in proptest::collection::vec(any::<u32>(), 0..=16),
        pad in proptest::collection::vec(any::<u32>(), 1..=8),
    ) {
        let mut target = source.clone();
        target.extend_from_slice(&pad);
        let original_tail = target[source.len()..].to_vec();
        cpu_ref(&mut target, &source);
        prop_assert_eq!(&target[..source.len()], &source[..]);
        prop_assert_eq!(&target[source.len()..], &original_tail[..]);
    }

    #[test]
    fn copy_into_smaller_target_truncates(
        source in proptest::collection::vec(any::<u32>(), 2..=32),
    ) {
        let target_len = source.len() - 1;
        let mut target = vec![0u32; target_len];
        cpu_ref(&mut target, &source);
        prop_assert_eq!(target, &source[..target_len]);
    }

    #[test]
    fn copy_from_empty_is_no_op(
        target in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let mut target = target;
        let original = target.clone();
        cpu_ref(&mut target, &[]);
        prop_assert_eq!(target, original);
    }
}

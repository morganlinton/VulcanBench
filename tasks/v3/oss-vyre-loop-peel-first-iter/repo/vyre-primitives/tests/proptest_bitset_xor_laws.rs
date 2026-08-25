//! Property gates for `vyre_primitives::bitset::xor::cpu_ref`.

#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::xor::cpu_ref;

fn manual_xor(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    lhs.iter().zip(rhs.iter()).map(|(a, b)| a ^ b).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn cpu_ref_matches_manual_xor(lhs in proptest::collection::vec(any::<u32>(), 0..=32), rhs in proptest::collection::vec(any::<u32>(), 0..=32)) {
        prop_assert_eq!(cpu_ref(&lhs, &rhs), manual_xor(&lhs, &rhs));
    }

    #[test]
    fn xor_is_commutative(lhs in proptest::collection::vec(any::<u32>(), 1..=16), rhs in proptest::collection::vec(any::<u32>(), 1..=16)) {
        let n = lhs.len().min(rhs.len());
        let l = &lhs[..n];
        let r = &rhs[..n];
        prop_assert_eq!(cpu_ref(l, r), cpu_ref(r, l));
    }

    #[test]
    fn xor_with_zero_rhs_is_identity(lhs in proptest::collection::vec(any::<u32>(), 1..=16)) {
        let zero = vec![0u32; lhs.len()];
        prop_assert_eq!(cpu_ref(&lhs, &zero), lhs);
    }

    #[test]
    fn xor_with_self_is_zero(v in proptest::collection::vec(any::<u32>(), 0..=16)) {
        let zero = vec![0u32; v.len()];
        prop_assert_eq!(cpu_ref(&v, &v), zero);
    }

    #[test]
    fn xor_is_involutive(a in proptest::collection::vec(any::<u32>(), 1..=16), b in proptest::collection::vec(any::<u32>(), 1..=16)) {
        let n = a.len().min(b.len());
        let x = cpu_ref(&a[..n], &b[..n]);
        let y = cpu_ref(&x, &b[..n]);
        prop_assert_eq!(y, a[..n].to_vec());
    }
}

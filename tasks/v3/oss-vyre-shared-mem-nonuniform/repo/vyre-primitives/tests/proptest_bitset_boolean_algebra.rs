//! Property gates for bitset boolean algebra across AND, OR, and NOT.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::bitset::{and, equal, not, or};

fn same(left: &[u32], right: &[u32]) -> bool {
    equal::cpu_ref(left, right) == 1
}

fn split_pairs(pairs: Vec<(u32, u32)>) -> (Vec<u32>, Vec<u32>) {
    pairs.into_iter().unzip()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn and_or_are_commutative(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
        b in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        prop_assert_eq!(and::cpu_ref(&a, &b), and::cpu_ref(&b, &a), "bitset AND must be commutative");
        prop_assert_eq!(or::cpu_ref(&a, &b), or::cpu_ref(&b, &a), "bitset OR must be commutative");
    }

    #[test]
    fn and_or_are_associative(
        a in proptest::collection::vec(any::<u32>(), 0..=8),
        b in proptest::collection::vec(any::<u32>(), 0..=8),
        c in proptest::collection::vec(any::<u32>(), 0..=8),
    ) {
        let and_ab_c = and::cpu_ref(&and::cpu_ref(&a, &b), &c);
        let and_a_bc = and::cpu_ref(&a, &and::cpu_ref(&b, &c));
        prop_assert_eq!(and_ab_c, and_a_bc, "bitset AND must be associative");

        let or_ab_c = or::cpu_ref(&or::cpu_ref(&a, &b), &c);
        let or_a_bc = or::cpu_ref(&a, &or::cpu_ref(&b, &c));
        prop_assert_eq!(or_ab_c, or_a_bc, "bitset OR must be associative");
    }

    #[test]
    fn and_or_are_idempotent(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        prop_assert!(same(&a, &and::cpu_ref(&a, &a)), "a & a must equal a");
        prop_assert!(same(&a, &or::cpu_ref(&a, &a)), "a | a must equal a");
    }

    #[test]
    fn identities_and_annihilators_hold(
        a in proptest::collection::vec(any::<u32>(), 0..=16),
    ) {
        let zeros = vec![0u32; a.len()];
        let ones = vec![0xFFFF_FFFFu32; a.len()];

        prop_assert!(same(&a, &and::cpu_ref(&a, &ones)), "a & 1 must equal a");
        prop_assert!(same(&zeros, &and::cpu_ref(&a, &zeros)), "a & 0 must equal 0");
        prop_assert!(same(&a, &or::cpu_ref(&a, &zeros)), "a | 0 must equal a");
        prop_assert!(same(&ones, &or::cpu_ref(&a, &ones)), "a | 1 must equal 1");
    }

    #[test]
    fn distributive_laws_hold(
        a in proptest::collection::vec(any::<u32>(), 0..=8),
        b in proptest::collection::vec(any::<u32>(), 0..=8),
        c in proptest::collection::vec(any::<u32>(), 0..=8),
    ) {
        let and_over_or_left = and::cpu_ref(&or::cpu_ref(&a, &b), &c);
        let and_over_or_right = or::cpu_ref(&and::cpu_ref(&a, &c), &and::cpu_ref(&b, &c));
        prop_assert_eq!(and_over_or_left, and_over_or_right, "(a | b) & c must equal (a & c) | (b & c)");

        let or_over_and_left = or::cpu_ref(&and::cpu_ref(&a, &b), &c);
        let or_over_and_right = and::cpu_ref(&or::cpu_ref(&a, &c), &or::cpu_ref(&b, &c));
        prop_assert_eq!(or_over_and_left, or_over_and_right, "(a & b) | c must equal (a | c) & (b | c)");
    }

    #[test]
    fn absorption_laws_hold(
        pairs in proptest::collection::vec(any::<(u32, u32)>(), 0..=16),
    ) {
        let (a, b) = split_pairs(pairs);

        prop_assert!(same(&a, &and::cpu_ref(&a, &or::cpu_ref(&a, &b))), "a & (a | b) must equal a");
        prop_assert!(same(&a, &or::cpu_ref(&a, &and::cpu_ref(&a, &b))), "a | (a & b) must equal a");
    }

    #[test]
    fn de_morgan_laws_hold_for_equal_width_inputs(
        pairs in proptest::collection::vec(any::<(u32, u32)>(), 0..=16),
    ) {
        let (a, b) = split_pairs(pairs);

        let not_and = not::cpu_ref(&and::cpu_ref(&a, &b));
        let not_a_or_not_b = or::cpu_ref(&not::cpu_ref(&a), &not::cpu_ref(&b));
        prop_assert!(same(&not_and, &not_a_or_not_b), "!(a & b) must equal !a | !b");

        let not_or = not::cpu_ref(&or::cpu_ref(&a, &b));
        let not_a_and_not_b = and::cpu_ref(&not::cpu_ref(&a), &not::cpu_ref(&b));
        prop_assert!(same(&not_or, &not_a_and_not_b), "!(a | b) must equal !a & !b");
    }
}

//! Failure-oriented adversarial integration tests for bitset primitives.
//!
//! Coverage: and_not, popcount, test_bit  -  hostile boundaries, empty
//! bitsets, cross-word node indices, alternating patterns.
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use vyre_primitives::bitset::and_not::cpu_ref as and_not_cpu_ref;
use vyre_primitives::bitset::popcount::cpu_ref as popcount_cpu_ref;
use vyre_primitives::bitset::test_bit::cpu_ref as test_bit_cpu_ref;

// ---------------------------------------------------------------------------
// and_not
// ---------------------------------------------------------------------------

#[test]
fn and_not_empty() {
    assert_eq!(and_not_cpu_ref(&[], &[]), Vec::<u32>::new());
}

#[test]
fn and_not_a_eq_b_is_zero() {
    let a = vec![0xDEAD_BEEF, 0x0F0F_0F0F];
    assert_eq!(and_not_cpu_ref(&a, &a), vec![0, 0]);
}

#[test]
fn and_not_b_all_ones_is_zero() {
    let lhs = vec![0xFFFF_FFFF, 0xFFFF_FFFF];
    let rhs = vec![0xFFFF_FFFF, 0xFFFF_FFFF];
    assert_eq!(and_not_cpu_ref(&lhs, &rhs), vec![0, 0]);
}

#[test]
fn and_not_cross_word_boundary() {
    let lhs = vec![0x8000_0000, 0x0000_0001];
    let rhs = vec![0x0000_0000, 0x0000_0000];
    assert_eq!(and_not_cpu_ref(&lhs, &rhs), vec![0x8000_0000, 0x0000_0001]);
}

// ---------------------------------------------------------------------------
// popcount
// ---------------------------------------------------------------------------

#[test]
fn popcount_empty() {
    assert_eq!(popcount_cpu_ref(&[]), Vec::<u32>::new());
}

#[test]
fn popcount_all_zeros() {
    assert_eq!(popcount_cpu_ref(&[0, 0, 0]), vec![0, 0, 0]);
}

#[test]
fn popcount_all_ones() {
    assert_eq!(popcount_cpu_ref(&[0xFFFF_FFFF, 0xFFFF_FFFF]), vec![32, 32]);
}

#[test]
fn popcount_alternating() {
    assert_eq!(popcount_cpu_ref(&[0xAAAA_AAAA]), vec![16]);
    assert_eq!(popcount_cpu_ref(&[0x5555_5555]), vec![16]);
}

#[test]
fn popcount_cross_word_boundary() {
    assert_eq!(popcount_cpu_ref(&[0x8000_0000, 0x0000_0001]), vec![1, 1]);
}

// ---------------------------------------------------------------------------
// test_bit
// ---------------------------------------------------------------------------

#[test]
fn test_bit_empty() {
    assert_eq!(test_bit_cpu_ref(&[], 0), 0);
    assert_eq!(test_bit_cpu_ref(&[], 31), 0);
    assert_eq!(test_bit_cpu_ref(&[], 32), 0);
}

#[test]
fn test_bit_single_word_all_bits() {
    let word = 0xFFFF_FFFF;
    for bit in 0..32 {
        assert_eq!(test_bit_cpu_ref(&[word], bit), 1);
    }
}

#[test]
fn test_bit_cross_word_boundary() {
    let buf = vec![0x8000_0000, 0x0000_0001];
    assert_eq!(test_bit_cpu_ref(&buf, 31), 1);
    assert_eq!(test_bit_cpu_ref(&buf, 32), 1);
    assert_eq!(test_bit_cpu_ref(&buf, 30), 0);
    assert_eq!(test_bit_cpu_ref(&buf, 33), 0);
}

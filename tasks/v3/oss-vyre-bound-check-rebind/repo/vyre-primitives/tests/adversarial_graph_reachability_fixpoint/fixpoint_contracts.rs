use super::*;

#[test]
fn fixpoint_reference_eval_equal_is_zero() {
    assert_eq!(reference_eval(&[0b1010], &[0b1010]), 0);
    assert_eq!(reference_eval(&[0xFFFF_FFFF; 16], &[0xFFFF_FFFF; 16]), 0);
    assert_eq!(reference_eval(&[], &[]), 0);
}

#[test]
fn fixpoint_reference_eval_different_is_one() {
    assert_eq!(reference_eval(&[0b1010], &[0b1011]), 1);
    assert_eq!(reference_eval(&[0; 16], &[1; 16]), 1);
}

#[test]
fn fixpoint_reference_eval_mismatched_lengths_is_one() {
    assert_eq!(reference_eval(&[0, 0], &[0]), 1);
}

#[test]
fn fixpoint_warm_start_zero_seed_equals_cold() {
    let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0001], &[0]);
    assert_eq!(updated, vec![0b0001]);
    assert_eq!(flag, 0);
}

#[test]
fn fixpoint_warm_start_seed_overwrites_current() {
    let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b1111]);
    assert_eq!(updated, vec![0b1111]);
    assert_eq!(flag, 1, "c0 (0b0001) != next (0b0011) → flag must be 1");
}

#[test]
fn fixpoint_warm_start_empty_bitsets() {
    let (updated, flag) = reference_eval_warm_start(&[], &[], &[]);
    assert!(updated.is_empty());
    assert_eq!(flag, 0);
}

#[test]
fn fixpoint_warm_start_large_bitsets() {
    let current = vec![0xAAAAAAAAu32; 1024];
    let next = vec![0xBBBBBBBBu32; 1024];
    let seed = vec![0x11111111u32; 1024];
    let (updated, flag) = reference_eval_warm_start(&current, &next, &seed);
    assert_eq!(updated.len(), 1024);
    assert_eq!(updated[0], 0xBBBBBBBB);
    assert_eq!(flag, 1);
}

#[test]
fn fixpoint_monotonic_growth_invariant() {
    // Simulate two fixpoint steps: current0 -> next0 -> current1
    let current0 = vec![0b0001u32];
    let next0 = vec![0b0011u32];
    let next1 = vec![0b0011u32]; // no further growth

    let flag0 = reference_eval(&current0, &next0);
    assert_eq!(flag0, 1, "first step must signal change");

    let flag1 = reference_eval(&next0, &next1);
    assert_eq!(flag1, 0, "second step must signal convergence");
}

#[test]
fn fixpoint_idempotence_after_convergence() {
    let converged = vec![0b1111u32];
    let flag = reference_eval(&converged, &converged);
    assert_eq!(flag, 0, "identical inputs must always yield 0");
}

#[test]
fn fixpoint_warm_start_anticipates_transfer() {
    // current = 0b0001, transfer says next = 0b0011, seed = 0b0010 anticipates delta.
    // c0 != next → flag must still be 1 because transfer added new bits.
    let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b0010]);
    assert_eq!(updated, vec![0b0011]);
    assert_eq!(flag, 1);
}

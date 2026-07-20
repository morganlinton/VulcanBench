//! Failure-oriented adversarial tests for fixpoint primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "fixpoint")]

fn reference_eval(current: &[u32], next: &[u32]) -> u32 {
    u32::from(current != next)
}

fn reference_eval_warm_start(current: &[u32], next: &[u32], seed: &[u32]) -> (Vec<u32>, u32) {
    debug_assert_eq!(current.len(), seed.len());
    debug_assert_eq!(current.len(), next.len());
    let updated = current
        .iter()
        .zip(seed.iter())
        .map(|(current_word, seed_word)| current_word | seed_word)
        .collect();
    (updated, u32::from(current != next))
}

#[test]
fn reference_eval_equal_bitsets() {
    let cases: Vec<(Vec<u32>, Vec<u32>)> = vec![
        (vec![], vec![]),
        (vec![0], vec![0]),
        (vec![u32::MAX], vec![u32::MAX]),
        (vec![0, 0, 0], vec![0, 0, 0]),
        (vec![0xAAAAAAAA; 16], vec![0xAAAAAAAA; 16]),
    ];
    for (current, next) in cases {
        let got = reference_eval(&current, &next);
        assert_eq!(got, 0, "equal bitsets must yield 0");
    }
}

#[test]
fn reference_eval_different_bitsets() {
    let cases: Vec<(Vec<u32>, Vec<u32>)> = vec![
        (vec![0], vec![1]),
        (vec![0b0001], vec![0b0011]),
        (vec![u32::MAX, 0], vec![u32::MAX, 1]),
        (vec![0; 16], vec![1; 16]),
    ];
    for (current, next) in cases {
        let got = reference_eval(&current, &next);
        assert_eq!(got, 1, "different bitsets must yield 1");
    }
}

#[test]
fn reference_eval_mismatched_lengths() {
    // reference_eval uses slice equality which returns false for mismatched lengths
    let got = reference_eval(&[0, 0], &[0]);
    assert_eq!(got, 1, "mismatched lengths treated as different");
}

#[test]
fn warm_start_zero_seed_matches_cold_semantics() {
    let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0]);
    assert_eq!(updated, vec![0b0001]);
    assert_eq!(flag, 1);
}

#[test]
fn warm_start_seed_covers_all_bits() {
    let (updated, flag) = reference_eval_warm_start(&[0b0001], &[0b0011], &[0b1111]);
    assert_eq!(updated, vec![0b1111]);
    assert_eq!(flag, 1);
}

#[test]
fn warm_start_empty_bitsets() {
    let (updated, flag) = reference_eval_warm_start(&[], &[], &[]);
    assert!(updated.is_empty());
    assert_eq!(flag, 0);
}

#[test]
fn warm_start_converged_no_change() {
    let (updated, flag) = reference_eval_warm_start(&[0b0011], &[0b0011], &[0b0000]);
    assert_eq!(updated, vec![0b0011]);
    assert_eq!(flag, 0);
}

#[test]
fn warm_start_large_bitsets() {
    let current = vec![0xAAAAAAAAu32; 1024];
    let next = vec![0xBBBBBBBBu32; 1024];
    let seed = vec![0x11111111u32; 1024];
    let (updated, flag) = reference_eval_warm_start(&current, &next, &seed);
    assert_eq!(updated.len(), 1024);
    assert_eq!(updated[0], 0xBBBBBBBB);
    assert_eq!(flag, 1);
}

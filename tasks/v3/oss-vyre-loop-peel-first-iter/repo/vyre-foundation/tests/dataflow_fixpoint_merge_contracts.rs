//! Generated contracts for shared dataflow-fixpoint merge kernels.

use vyre_foundation::pass_substrate::dataflow_fixpoint::{merge_min_changed, merge_or_changed};

fn generated_word(seed: u32, index: u32) -> u32 {
    let mixed =
        seed.wrapping_mul(0x9E37_79B9).rotate_left(index % 31) ^ index.wrapping_mul(0x85EB_CA6B);
    mixed ^ mixed.rotate_right(7)
}

#[test]
fn generated_or_merge_matches_bitwise_join_and_change_flag() {
    for seed in 0..128u32 {
        let len = (seed as usize % 33) + 1;
        let mut current = (0..len as u32)
            .map(|index| generated_word(seed, index))
            .collect::<Vec<_>>();
        let next = (0..len as u32)
            .map(|index| generated_word(seed ^ 0xA5A5_5A5A, index))
            .collect::<Vec<_>>();
        let expected = current
            .iter()
            .zip(next.iter())
            .map(|(left, right)| *left | *right)
            .collect::<Vec<_>>();
        let expected_changed = current.iter().zip(expected.iter()).any(|(a, b)| a != b);

        let changed = merge_or_changed(&mut current, &next);

        assert_eq!(
            current, expected,
            "Fix: OR merge must compute lattice join."
        );
        assert_eq!(
            changed, expected_changed,
            "Fix: OR merge changed flag must report exactly whether the destination mutated."
        );
        assert!(
            !merge_or_changed(&mut current, &next),
            "Fix: OR merge must be idempotent at fixpoint."
        );
    }
}

#[test]
fn generated_min_merge_matches_shortest_path_join_and_change_flag() {
    for seed in 0..128u32 {
        let len = (seed as usize % 29) + 1;
        let mut current = (0..len as u32)
            .map(|index| generated_word(seed | 1, index))
            .collect::<Vec<_>>();
        let next = (0..len as u32)
            .map(|index| generated_word(seed ^ 0x5A5A_A5A5, index))
            .collect::<Vec<_>>();
        let expected = current
            .iter()
            .zip(next.iter())
            .map(|(left, right)| (*left).min(*right))
            .collect::<Vec<_>>();
        let expected_changed = current.iter().zip(expected.iter()).any(|(a, b)| a != b);

        let changed = merge_min_changed(&mut current, &next);

        assert_eq!(
            current, expected,
            "Fix: Min merge must compute min-plus closure join."
        );
        assert_eq!(
            changed, expected_changed,
            "Fix: Min merge changed flag must report exactly whether the destination mutated."
        );
        assert!(
            !merge_min_changed(&mut current, &next),
            "Fix: Min merge must be idempotent at fixpoint."
        );
    }
}

#[test]
fn adversarial_merge_extremes_are_exact() {
    let mut or_current = [0, u32::MAX, 0xAAAA_AAAA, 0x5555_5555];
    let or_next = [u32::MAX, 0, 0x5555_5555, 0xAAAA_AAAA];
    assert!(merge_or_changed(&mut or_current, &or_next));
    assert_eq!(or_current, [u32::MAX; 4]);
    assert!(!merge_or_changed(&mut or_current, &or_next));

    let mut min_current = [u32::MAX, 0, 7, 42];
    let min_next = [0, u32::MAX, 9, 1];
    assert!(merge_min_changed(&mut min_current, &min_next));
    assert_eq!(min_current, [0, 0, 7, 1]);
    assert!(!merge_min_changed(&mut min_current, &min_next));
}

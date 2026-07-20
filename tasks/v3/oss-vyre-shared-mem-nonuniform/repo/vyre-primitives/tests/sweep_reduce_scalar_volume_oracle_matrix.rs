//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::{all, any, max, min, sum};

fn lcg_u32(seed: u32, len: usize) -> Vec<u32> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx % 31) as u32);
            state ^ (idx as u32).wrapping_mul(0x85EB_CA6B)
        })
        .collect()
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_any_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 200));
        let expected = u32::from(input.iter().any(|value| *value != 0));
        assert_eq!(
            any::cpu_ref(&input),
            expected,
            "Fix: reduce_any volume case {idx}"
        );
    }
}

#[test]
fn sweep_reduce_all_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 200));
        let expected = u32::from(input.iter().all(|value| *value != 0));
        assert_eq!(
            all::cpu_ref(&input),
            expected,
            "Fix: reduce_all volume case {idx}"
        );
    }
}

#[test]
fn sweep_reduce_sum_scalar_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 200));
        let expected = input.iter().copied().fold(0u32, u32::wrapping_add);
        assert_eq!(
            sum::cpu_ref(&input),
            expected,
            "Fix: reduce_sum scalar volume case {idx}"
        );
    }
}

#[test]
fn sweep_reduce_min_scalar_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 200));
        let expected = input.iter().copied().min().unwrap_or(u32::MAX);
        assert_eq!(
            min::cpu_ref(&input),
            expected,
            "Fix: reduce_min scalar volume case {idx}"
        );
    }
}

#[test]
fn sweep_reduce_max_scalar_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 200));
        let expected = input.iter().copied().max().unwrap_or(0);
        assert_eq!(
            max::cpu_ref(&input),
            expected,
            "Fix: reduce_max scalar volume case {idx}"
        );
    }
}

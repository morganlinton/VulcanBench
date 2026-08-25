//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::range_counts;

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

fn oracle(histogram: &[u32], start: u32, end: u32) -> u32 {
    let start = start as usize;
    let end = end.min(histogram.len() as u32) as usize;
    if start >= end {
        0
    } else {
        histogram[start..end].iter().sum()
    }
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_range_counts_volume_oracle_matrix() {
    for idx in 0..CASES {
        let histogram = lcg_u32(idx as u32, 8 + (idx % 24));
        let start = (idx % 8) as u32;
        let end = start + 1 + ((idx >> 3) % 8) as u32;
        let expected = oracle(&histogram, start, end);
        let actual = range_counts::cpu_ref(&histogram, start, end);

        assert_eq!(
            actual, expected,
            "Fix: reduce_range_counts volume case {idx}"
        );
    }
}

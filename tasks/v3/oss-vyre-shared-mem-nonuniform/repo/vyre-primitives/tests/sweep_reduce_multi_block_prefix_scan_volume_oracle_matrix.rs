//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::multi_block_prefix_scan;

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

fn oracle_inclusive(input: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(input.len());
    let mut acc = 0u32;
    for &x in input {
        acc = acc.wrapping_add(x);
        out.push(acc);
    }
    out
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_multi_block_prefix_scan_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, idx % 200);
        assert_eq!(
            multi_block_prefix_scan::cpu_ref(&input),
            oracle_inclusive(&input),
            "Fix: multi_block_prefix_scan volume case {idx}"
        );
    }
}

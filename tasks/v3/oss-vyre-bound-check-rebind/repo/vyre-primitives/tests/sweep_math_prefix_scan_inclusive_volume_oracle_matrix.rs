//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "math", feature = "cpu-parity"))]

use vyre_primitives::math::prefix_scan::{cpu_ref, ScanKind};

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

fn oracle_inclusive_scan(input: &[u32]) -> Vec<u32> {
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
fn sweep_math_prefix_scan_inclusive_volume_oracle_matrix() {
    for idx in 0..CASES {
        let len = idx % 256;
        let input = lcg_u32(idx as u32, len);
        assert_eq!(
            cpu_ref(&input, ScanKind::InclusiveSum),
            oracle_inclusive_scan(&input),
            "Fix: prefix_scan inclusive volume case {idx} len={len}"
        );
    }
}

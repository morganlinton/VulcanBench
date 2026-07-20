//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::gather;

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

fn oracle(src: &[u32], indices: &[u32]) -> Vec<u32> {
    indices
        .iter()
        .map(|&i| {
            if (i as usize) < src.len() {
                src[i as usize]
            } else {
                0
            }
        })
        .collect()
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_gather_volume_oracle_matrix() {
    for idx in 0..CASES {
        let src = lcg_u32(idx as u32, 1 + (idx % 64));
        let indices = lcg_u32(idx as u32 ^ 0x1DEF0001, 1 + (idx % 64));
        let expected = oracle(&src, &indices);
        let actual = gather::cpu_ref(&src, &indices);

        assert_eq!(actual, expected, "Fix: reduce_gather volume case {idx}");
    }
}

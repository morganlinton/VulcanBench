//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::scatter;

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

fn oracle(src: &[u32], indices: &[u32], dst_len: usize) -> Vec<u32> {
    let mut dst = vec![0u32; dst_len];
    for (value, &index) in src.iter().zip(indices) {
        if (index as usize) < dst_len {
            dst[index as usize] = *value;
        }
    }
    dst
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_scatter_volume_oracle_matrix() {
    for idx in 0..CASES {
        let src = lcg_u32(idx as u32, 1 + (idx % 48));
        let indices = lcg_u32(idx as u32 ^ 0x5CA70001, 1 + (idx % 48));
        let dst_len = 1 + (idx % 64);
        let expected = oracle(&src, &indices, dst_len);
        let actual = scatter::cpu_ref(&src, &indices, dst_len);

        assert_eq!(actual, expected, "Fix: reduce_scatter volume case {idx}");
    }
}

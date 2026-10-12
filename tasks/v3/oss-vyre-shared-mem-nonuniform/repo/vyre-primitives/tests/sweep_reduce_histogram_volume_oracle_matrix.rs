//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::histogram;

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

fn oracle(input: &[u32], num_bins: u32) -> Vec<u32> {
    let mut out = vec![0u32; num_bins as usize];
    for &bin in input {
        if let Ok(b) = usize::try_from(bin) {
            if b < out.len() {
                out[b] = out[b].saturating_add(1);
            }
        }
    }
    out
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_histogram_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 96));
        let num_bins = (4 + (idx % 32)) as u32;
        let expected = oracle(&input, num_bins);
        let actual = histogram::cpu_ref(&input, num_bins);

        assert_eq!(actual, expected, "Fix: reduce_histogram volume case {idx}");
    }
}

//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use vyre_primitives::bitset::contains;

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

fn binary_pairs(cases: usize) -> impl Iterator<Item = (Vec<u32>, Vec<u32>)> {
    (0..cases).map(|case| {
        let seed = case as u64 ^ 0xB175_E7B1_7500_0000;
        let lhs_len = 1 + ((seed >> 3) as usize % 129);
        let rhs_len = 1 + ((seed >> 11) as usize % 129);
        (
            lcg_u32(seed as u32, lhs_len),
            lcg_u32(seed.rotate_left(17) as u32 ^ 0xD00D_F00D, rhs_len),
        )
    })
}

fn oracle(buf: &[u32], index: u32) -> u32 {
    let w = (index / 32) as usize;
    let b = index % 32;
    if w < buf.len() {
        (buf[w] >> b) & 1
    } else {
        0
    }
}

const CASES: usize = 16384;

#[test]
fn sweep_bitset_contains_volume_oracle_matrix() {
    for (idx, (buf, _)) in binary_pairs(CASES).enumerate() {
        let index = (idx as u32).wrapping_mul(0x9E37_79B9) % (buf.len() as u32 * 32 + 17);
        assert_eq!(
            contains::cpu_ref(&buf, index),
            oracle(&buf, index),
            "Fix: bitset_contains volume case {idx}"
        );
    }
}

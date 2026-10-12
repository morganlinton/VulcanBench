//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use vyre_primitives::bitset::subset_of;

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

fn oracle(lhs: &[u32], rhs: &[u32]) -> u32 {
    let n = lhs.len().min(rhs.len());
    for i in 0..n {
        if (lhs[i] & !rhs[i]) != 0 {
            return 0;
        }
    }
    if lhs.len() > rhs.len() {
        return u32::from(lhs[n..].iter().all(|&w| w == 0));
    }
    1
}

const CASES: usize = 16384;

#[test]
fn sweep_bitset_subset_of_volume_oracle_matrix() {
    for (idx, (lhs, rhs)) in binary_pairs(CASES).enumerate() {
        let expected = oracle(&lhs, &rhs);
        let actual = subset_of::cpu_ref(&lhs, &rhs);
        assert_eq!(
            actual,
            expected,
            "Fix: bitset_subset_of volume case {idx} lhs_len={} rhs_len={}",
            lhs.len(),
            rhs.len()
        );
    }
}

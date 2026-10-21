//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "bitset", feature = "cpu-parity"))]

use vyre_primitives::bitset::copy;

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
fn sweep_bitset_copy_volume_oracle_matrix() {
    for idx in 0..CASES {
        let len = 1 + (idx % 128);
        let source = lcg_u32(idx as u32, len);
        let mut target = lcg_u32(idx as u32 ^ 0xC0FF0001, len);
        let mut expected = target.clone();
        copy::cpu_ref(&mut target, &source);
        expected.copy_from_slice(&source);
        assert_eq!(target, expected, "Fix: bitset_copy volume case {idx}");
    }
}

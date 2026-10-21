//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "reduce", feature = "cpu-parity"))]

use vyre_primitives::reduce::workgroup_any;

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

fn oracle(values: &[u32]) -> u32 {
    u32::from(values.iter().any(|&v| v != 0))
}

const CASES: usize = 16384;

#[test]
fn sweep_reduce_workgroup_any_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = lcg_u32(idx as u32, 1 + (idx % 256));
        let expected = oracle(&input);
        let actual = workgroup_any::cpu_ref(&input);
        assert_eq!(
            actual, expected,
            "Fix: reduce_workgroup_any volume case {idx}"
        );
    }
}

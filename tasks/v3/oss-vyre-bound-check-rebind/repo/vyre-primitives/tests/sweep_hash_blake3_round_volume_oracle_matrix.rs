//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(all(feature = "hash", feature = "cpu-parity"))]

use vyre_primitives::hash::blake3::{cpu_blake3_round, MSG_SCHEDULE};

const CASES: usize = 16384;

fn oracle_blake3_g(
    state: &mut [u32; 16],
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    mx: u32,
    my: u32,
) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn oracle_blake3_round(state: &mut [u32; 16], message: &[u32; 16], perm: &[usize; 16]) {
    let mut m = [0u32; 16];
    for (i, &src) in perm.iter().enumerate() {
        m[i] = message[src];
    }
    oracle_blake3_g(state, 0, 4, 8, 12, m[0], m[1]);
    oracle_blake3_g(state, 1, 5, 9, 13, m[2], m[3]);
    oracle_blake3_g(state, 2, 6, 10, 14, m[4], m[5]);
    oracle_blake3_g(state, 3, 7, 11, 15, m[6], m[7]);
    oracle_blake3_g(state, 0, 5, 10, 15, m[8], m[9]);
    oracle_blake3_g(state, 1, 6, 11, 12, m[10], m[11]);
    oracle_blake3_g(state, 2, 7, 8, 13, m[12], m[13]);
    oracle_blake3_g(state, 3, 4, 9, 14, m[14], m[15]);
}

#[test]
fn sweep_hash_blake3_round_volume_oracle_matrix() {
    for idx in 0..CASES {
        let mut message = [0u32; 16];
        for lane in 0..16 {
            message[lane] = (idx as u32).wrapping_mul(lane as u32 + 1);
        }
        let mut expected = message;
        let mut actual = message;
        let perm = &MSG_SCHEDULE[(idx % MSG_SCHEDULE.len()) as usize];
        oracle_blake3_round(&mut expected, &message, perm);
        cpu_blake3_round(&mut actual, &message, perm);
        assert_eq!(actual, expected, "Fix: blake3_round volume case {idx}");
    }
}

//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(feature = "hash")]

use vyre_primitives::hash::adler32;

const ADLER_MOD: u32 = 65_521;
const CASES: usize = 16384;

fn oracle_adler32(bytes: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in bytes {
        a = (a + u32::from(byte)) % ADLER_MOD;
        b = (b + a) % ADLER_MOD;
    }
    (b << 16) | a
}

fn hostile_bytes(seed: u32) -> Vec<u8> {
    let len = 1 + (seed as usize % 512);
    let mut v = Vec::with_capacity(len);
    let mut s = seed as u64 ^ 0xDEAD_BEEF_CAFE_BABE;
    for _ in 0..len {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        v.push(s as u8);
    }
    v
}

#[test]
fn sweep_hash_adler32_volume_oracle_matrix() {
    for idx in 0..CASES {
        let bytes = hostile_bytes(idx as u32);
        assert_eq!(
            adler32::adler32(&bytes),
            oracle_adler32(&bytes),
            "Fix: adler32 volume case {idx} len={}",
            bytes.len()
        );
    }
}

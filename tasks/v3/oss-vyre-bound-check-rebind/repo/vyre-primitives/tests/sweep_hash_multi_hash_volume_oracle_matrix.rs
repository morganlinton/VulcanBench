//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(feature = "hash")]

use vyre_primitives::hash::{adler32, crc32, fnv1a, multi_hash};

const CASES: usize = 16384;

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

fn oracle_multi(bytes: &[u8]) -> (u32, u32, u32) {
    (
        crc32::crc32(bytes),
        fnv1a::fnv1a32(bytes),
        adler32::adler32(bytes),
    )
}

#[test]
fn sweep_hash_multi_hash_volume_oracle_matrix() {
    for idx in 0..CASES {
        let bytes = hostile_bytes(idx as u32 ^ 0xA11C_0DE1);
        let expected = oracle_multi(&bytes);
        let actual = multi_hash::multi_hash_reference(&bytes);
        assert_eq!(
            actual,
            expected,
            "Fix: multi_hash volume case {idx} len={}",
            bytes.len()
        );
    }
}

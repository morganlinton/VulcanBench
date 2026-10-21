//! Volume-wave oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(feature = "hash")]

use vyre_primitives::hash::{crc32, fnv1a};

fn oracle_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn oracle_fnv1a32(bytes: &[u8]) -> u32 {
    let mut h = 0x811c_9dc5u32;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn hostile_bytes() -> impl Iterator<Item = Vec<u8>> {
    (0..16384usize).map(|i| {
        let len = 1 + (i % 512);
        let mut v = Vec::with_capacity(len);
        let mut s = (i as u64) ^ 0xDEAD_BEEF_CAFE_BABE;
        for _ in 0..len {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.push(s as u8);
        }
        v
    })
}

#[test]
fn sweep_crc32_volume_oracle_matrix() {
    for (idx, bytes) in hostile_bytes().enumerate() {
        let expected = oracle_crc32(&bytes);
        let actual = crc32::crc32(&bytes);
        assert_eq!(
            actual,
            expected,
            "Fix: crc32 volume case {idx} len={}",
            bytes.len()
        );
    }
}

#[test]
fn sweep_fnv1a32_volume_oracle_matrix() {
    for (idx, bytes) in hostile_bytes().enumerate() {
        let expected = oracle_fnv1a32(&bytes);
        let actual = fnv1a::fnv1a32(&bytes);
        assert_eq!(
            actual,
            expected,
            "Fix: fnv1a32 volume case {idx} len={}",
            bytes.len()
        );
    }
}

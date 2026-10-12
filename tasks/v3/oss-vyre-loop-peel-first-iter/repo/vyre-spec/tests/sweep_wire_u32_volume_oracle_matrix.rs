//! Volume-wave oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]

use vyre_primitives::wire::{decode_u32_le_bytes_all, pack_u32_slice};

fn hostile_u32_vecs() -> impl Iterator<Item = Vec<u32>> {
    (0..16384usize).map(|i| {
        let n = 1 + (i % 64);
        (0..n)
            .map(|j| {
                (i as u32)
                    .wrapping_add(j as u32)
                    .rotate_left((j % 32) as u32)
            })
            .collect()
    })
}

#[test]
fn sweep_wire_u32_roundtrip_volume_oracle_matrix() {
    for (idx, words) in hostile_u32_vecs().enumerate() {
        let encoded = pack_u32_slice(&words);
        let decoded = decode_u32_le_bytes_all(&encoded);
        assert_eq!(
            decoded,
            words,
            "Fix: wire u32 roundtrip volume case {idx} len={}",
            words.len()
        );
    }
}

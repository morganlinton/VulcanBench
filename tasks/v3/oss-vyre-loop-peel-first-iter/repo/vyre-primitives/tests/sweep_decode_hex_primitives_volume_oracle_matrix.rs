//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(feature = "decode")]

use vyre_primitives::decode::hex::hex_decode_reference_packed;

const CASES: usize = 16384;
const ALPHABET: &[u8] = b"0123456789abcdefABCDEF";

fn oracle_hex_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for byte in b'0'..=b'9' {
        table[byte as usize] = u32::from(byte - b'0');
    }
    for byte in b'A'..=b'F' {
        table[byte as usize] = u32::from(byte - b'A' + 10);
    }
    for byte in b'a'..=b'f' {
        table[byte as usize] = u32::from(byte - b'a' + 10);
    }
    table
}

fn oracle_hex(input: &[u8]) -> Vec<u32> {
    let table = oracle_hex_table();
    input
        .chunks_exact(2)
        .map(|pair| (table[pair[0] as usize] << 4) | table[pair[1] as usize])
        .collect()
}

fn hostile_hex(seed: u32) -> Vec<u8> {
    let pairs = 1 + (seed % 64);
    let mut state = seed ^ 0x48EC_DECD;
    let mut out = Vec::with_capacity(pairs as usize * 2);
    for _ in 0..(pairs * 2) {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        out.push(ALPHABET[(state as usize) % ALPHABET.len()]);
    }
    out
}

#[test]
fn sweep_decode_hex_primitives_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = hostile_hex(idx as u32);
        assert_eq!(
            hex_decode_reference_packed(&input),
            oracle_hex(&input),
            "Fix: hex_decode primitives volume case {idx} len={}",
            input.len()
        );
    }
}

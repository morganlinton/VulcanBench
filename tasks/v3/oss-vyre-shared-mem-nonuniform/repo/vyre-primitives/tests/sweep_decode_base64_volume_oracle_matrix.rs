//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]
#![cfg(feature = "decode")]

use vyre_primitives::decode::base64::cpu_base64_decode;

const CASES: usize = 16384;
const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

fn build_std_table() -> [u8; 256] {
    let mut table = [0u8; 256];
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (idx, &ch) in alphabet.iter().enumerate() {
        table[ch as usize] = idx as u8;
    }
    table
}

fn oracle_base64(input: &[u8]) -> Vec<u8> {
    let table = build_std_table();
    let mut out = Vec::new();
    for chunk in input.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let mut accum = 0u32;
        let mut pads = 0u32;
        for &byte in chunk {
            if byte == b'=' {
                pads += 1;
                continue;
            }
            accum = (accum << 6) | u32::from(table[byte as usize]);
        }
        out.push((accum >> 16) as u8);
        if pads < 2 {
            out.push((accum >> 8) as u8);
        }
        if pads == 0 {
            out.push(accum as u8);
        }
    }
    out
}

fn hostile_b64(seed: u32) -> Vec<u8> {
    let quads = 1 + (seed % 48);
    let len = quads as usize * 4;
    let mut state = seed ^ 0xB64B_64B4;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        out.push(ALPHABET[(state as usize) % ALPHABET.len()]);
    }
    out
}

#[test]
fn sweep_decode_base64_volume_oracle_matrix() {
    for idx in 0..CASES {
        let input = hostile_b64(idx as u32);
        let expected = oracle_base64(&input);
        let actual = cpu_base64_decode(&input);
        assert_eq!(
            actual,
            expected,
            "Fix: base64_decode volume case {idx} len={}",
            input.len()
        );
    }
}

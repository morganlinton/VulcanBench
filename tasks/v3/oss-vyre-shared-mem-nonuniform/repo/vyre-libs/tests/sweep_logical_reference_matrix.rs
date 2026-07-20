//! Reference-vs-oracle matrix for logical elementwise ops.
//!
//! Each op is evaluated through `vyre_reference::reference_eval` and compared
//! against an independent bitwise scalar oracle over hostile `u32[4]` inputs.
//! This is intentionally deterministic matrix coverage, not proptest-only.

#![cfg(feature = "logical")]
#![allow(deprecated)]

use vyre_reference::value::Value;

fn bytes_from_u32(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn run_binary(program: &vyre::Program, a: &[u32; 4], b: &[u32; 4]) -> [u32; 4] {
    let outputs = vyre_reference::reference_eval(
        program,
        &[
            Value::from(bytes_from_u32(a)),
            Value::from(bytes_from_u32(b)),
            Value::from(vec![0_u8; 16]),
        ],
    )
    .unwrap_or_else(|error| panic!("Fix: logical reference run failed: {error}"));
    decode_u32x4(&outputs[0].to_bytes())
}

fn run_unary(program: &vyre::Program, input: &[u32; 4]) -> [u32; 4] {
    let outputs = vyre_reference::reference_eval(
        program,
        &[
            Value::from(bytes_from_u32(input)),
            Value::from(vec![0_u8; 16]),
        ],
    )
    .unwrap_or_else(|error| panic!("Fix: logical unary reference run failed: {error}"));
    decode_u32x4(&outputs[0].to_bytes())
}

fn decode_u32x4(raw: &[u8]) -> [u32; 4] {
    std::array::from_fn(|index| {
        let offset = index * 4;
        u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
    })
}

fn hostile_u32x4(seed: u32) -> [u32; 4] {
    let mut state = seed ^ 0xA5A5_5A5A;
    std::array::from_fn(|index| {
        state = state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .rotate_left((index as u32).wrapping_add(seed) & 15);
        match (seed.wrapping_add(index as u32)) % 8 {
            0 => state,
            1 => !state,
            2 => 0,
            3 => u32::MAX,
            4 => 0xAAAA_AAAA,
            5 => 0x5555_5555,
            6 => state & 0x0000_FFFF,
            _ => state | 0xFFFF_0000,
        }
    })
}

fn oracle_binary<F>(a: &[u32; 4], b: &[u32; 4], f: F) -> [u32; 4]
where
    F: Fn(u32, u32) -> u32,
{
    std::array::from_fn(|index| f(a[index], b[index]))
}

fn oracle_not(input: &[u32; 4]) -> [u32; 4] {
    std::array::from_fn(|index| !input[index])
}

#[test]
fn matrix_and_matches_bitwise_oracle() {
    let program = vyre_libs::logical::and("a", "b", "out", 4);
    let mut assertions = 0usize;
    for seed in 0..256_u32 {
        let a = hostile_u32x4(seed);
        let b = hostile_u32x4(seed.wrapping_mul(0x9E37_79B9));
        assert_eq!(
            run_binary(&program, &a, &b),
            oracle_binary(&a, &b, |lhs, rhs| lhs & rhs),
            "and seed={seed}"
        );
        assertions += 1;
    }
    assert_eq!(assertions, 256);
}

#[test]
fn matrix_or_matches_bitwise_oracle() {
    let program = vyre_libs::logical::or("a", "b", "out", 4);
    let mut assertions = 0usize;
    for seed in 0..256_u32 {
        let a = hostile_u32x4(seed ^ 0x0101_0101);
        let b = hostile_u32x4(seed.rotate_left(7));
        assert_eq!(
            run_binary(&program, &a, &b),
            oracle_binary(&a, &b, |lhs, rhs| lhs | rhs),
            "or seed={seed}"
        );
        assertions += 1;
    }
    assert_eq!(assertions, 256);
}

#[test]
fn matrix_xor_matches_bitwise_oracle() {
    let program = vyre_libs::logical::xor("a", "b", "out", 4);
    let mut assertions = 0usize;
    for seed in 0..256_u32 {
        let a = hostile_u32x4(seed ^ 0x1357_9BDF);
        let b = hostile_u32x4(seed.rotate_left(11) ^ 0x2468_ACE0);
        assert_eq!(
            run_binary(&program, &a, &b),
            oracle_binary(&a, &b, |lhs, rhs| lhs ^ rhs),
            "xor seed={seed}"
        );
        assertions += 1;
    }
    assert_eq!(assertions, 256);
}

#[test]
fn matrix_not_matches_bitwise_oracle() {
    use vyre_primitives::bitset::not::bitset_not;

    let program = bitset_not("input", "out", 4);
    let mut assertions = 0usize;
    for seed in 0..256_u32 {
        let input = hostile_u32x4(seed ^ 0xDEAD_BEEF);
        assert_eq!(
            run_unary(&program, &input),
            oracle_not(&input),
            "not seed={seed}"
        );
        assertions += 1;
    }
    assert_eq!(assertions, 256);
}

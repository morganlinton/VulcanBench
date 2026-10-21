//! Release sweep R2 - oracle matrix (handwritten reference, hostile corpus).
//! Generated scaffold - oracle logic is explicit; do not reduce to `assert!(is_ok)`.
#![forbid(unsafe_code)]

use vyre_primitives::hash::fnv1a;

fn oracle_fnv1a32(bytes: &[u8]) -> u32 {
    const OFFSET: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn oracle_fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn hostile_byte_slices() -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for len in [
        0usize, 1, 2, 3, 7, 15, 16, 17, 31, 32, 33, 63, 64, 127, 255, 256, 512, 1024, 2048,
    ] {
        out.push(vec![0u8; len]);
        out.push(vec![0xFFu8; len]);
        out.push((0..len as u8).collect());
        let mut alt = Vec::with_capacity(len);
        for i in 0..len {
            alt.push(if i % 2 == 0 { 0x55 } else { 0xAA });
        }
        out.push(alt);
    }
    let mut state = 0xC0FF_EE01u32;
    for len in [
        0, 1, 4, 8, 16, 32, 64, 128, 256, 512, 1023, 1024, 1025, 2047, 2048,
    ] {
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            v.push((state & 0xFF) as u8);
        }
        out.push(v);
    }
    out
}

#[test]
fn sweep_fnv1a32_oracle_covers_hostile_byte_corpus() {
    for (idx, bytes) in hostile_byte_slices().into_iter().enumerate() {
        let expected = oracle_fnv1a32(&bytes);
        let actual = fnv1a::fnv1a32(&bytes);
        assert_eq!(actual, expected, "fnv1a32 case {idx} len={}", bytes.len());
        assert_eq!(fnv1a::fnv1a32_const(&bytes), expected, "const case {idx}");
    }
}

#[test]
fn sweep_fnv1a64_oracle_covers_hostile_byte_corpus() {
    for (idx, bytes) in hostile_byte_slices().into_iter().enumerate() {
        let expected = oracle_fnv1a64(&bytes);
        let actual = fnv1a::fnv1a64(&bytes);
        assert_eq!(actual, expected, "fnv1a64 case {idx} len={}", bytes.len());
    }
}

#[test]
fn sweep_fnv1a32_packed_low8_oracle_covers_word_corpus() {
    for (idx, words) in hostile_u32_words().into_iter().enumerate() {
        let expected = oracle_fnv1a32_packed(words.as_slice());
        let actual = fnv1a::fnv1a32_packed_u32_low8(words.as_slice());
        assert_eq!(actual, expected, "packed case {idx} len={}", words.len());
    }
}

fn oracle_fnv1a32_packed(words: &[u32]) -> u32 {
    let mut h = 0x811c_9dc5u32;
    for &w in words {
        let b = (w & 0xFF) as u8;
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn hostile_u32_words() -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    for len in [0usize, 1, 2, 7, 31, 32, 33, 127, 128, 255, 256, 1024] {
        out.push(vec![0; len]);
        out.push(vec![u32::MAX; len]);
        out.push(
            (0..len as u32)
                .map(|i| i.wrapping_mul(0x9E37_79B9))
                .collect(),
        );
    }
    let mut state = 0xDEAD_BEEFu32;
    for len in 0..=512usize {
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            state = state.rotate_left(5) ^ 0xA5A5_5A5A;
            v.push(state);
        }
        out.push(v);
    }
    out
}

//! Handwritten oracle matrix for `hash::crc32` and packed FNV-1a32 variants.
//!
//! Compares production CRC chunk/combine and FNV packed walkers against
//! independent byte-stream oracles across hostile lengths and LCG seeds.

#![forbid(unsafe_code)]
#![cfg(feature = "hash")]

use std::num::NonZeroU32;

use vyre_primitives::hash::crc32::{
    crc32, crc32_chunk, crc32_combine, crc32_combine_chunks, crc32_pair_reduce_chunks, Crc32Chunk,
    CRC32_INIT, CRC32_POLY,
};
use vyre_primitives::hash::fnv1a::{
    fnv1a32, fnv1a32_packed_u32_low8, FNV1A32_OFFSET, FNV1A32_PRIME,
};

#[test]
fn crc32_matches_independent_table_oracle_matrix() {
    for (case_idx, bytes) in byte_cases().iter().enumerate() {
        assert_eq!(
            crc32(bytes),
            oracle_crc32(bytes),
            "Fix: crc32 adversarial case {case_idx} len={} must match the independent table oracle.",
            bytes.len()
        );
    }
}

#[test]
fn crc32_chunk_and_combine_matches_independent_oracle_matrix() {
    for (case_idx, bytes) in byte_cases().iter().enumerate() {
        let chunk_size = NonZeroU32::new(1 + (case_idx % 64) as u32).expect("chunk size");
        let expected = oracle_crc32(bytes);
        let chunks: Vec<Crc32Chunk> = bytes
            .chunks(chunk_size.get() as usize)
            .map(|part| Crc32Chunk {
                len: part.len() as u64,
                crc: oracle_crc32(part),
            })
            .collect();
        let mut folded = chunks.clone();
        while folded.len() > 1 {
            folded = oracle_pair_reduce(&folded);
        }
        let actual_folded = {
            let chunks: Vec<Crc32Chunk> = if bytes.is_empty() {
                vec![Crc32Chunk {
                    len: 0,
                    crc: oracle_crc32(bytes),
                }]
            } else {
                bytes
                    .chunks(chunk_size.get() as usize)
                    .map(|part| Crc32Chunk {
                        len: part.len() as u64,
                        crc: oracle_crc32(part),
                    })
                    .collect()
            };
            let mut reduced = chunks;
            while reduced.len() > 1 {
                reduced = crc32_pair_reduce_chunks(&reduced)
                    .expect("Fix: generated CRC chunk lengths must not overflow.");
            }
            reduced[0]
        };
        assert_eq!(
            actual_folded.crc, expected,
            "Fix: crc32 map-reduce adversarial case {case_idx} must match direct CRC."
        );
        assert_eq!(
            actual_folded.len,
            bytes.len() as u64,
            "Fix: crc32 map-reduce adversarial case {case_idx} must preserve byte length."
        );

        for split in 0..=bytes.len().min(8) {
            let left = oracle_crc32(&bytes[..split]);
            let right = oracle_crc32(&bytes[split..]);
            assert_eq!(
                crc32_combine(left, right, (bytes.len() - split) as u64),
                expected,
                "Fix: crc32_combine adversarial case {case_idx} split={split} must match direct CRC."
            );
            let left_chunk = Crc32Chunk {
                len: split as u64,
                crc: left,
            };
            let right_chunk = Crc32Chunk {
                len: (bytes.len() - split) as u64,
                crc: right,
            };
            assert_eq!(
                crc32_combine_chunks(left_chunk, right_chunk)
                    .expect("Fix: generated CRC chunk length must not overflow.")
                    .crc,
                expected,
                "Fix: crc32_combine_chunks adversarial case {case_idx} split={split} must match direct CRC."
            );
        }

        assert_eq!(
            crc32_chunk(bytes),
            Crc32Chunk {
                len: bytes.len() as u64,
                crc: expected,
            },
            "Fix: crc32_chunk adversarial case {case_idx} must match independent oracle."
        );
    }
}

#[test]
fn fnv1a32_packed_low8_matches_independent_oracle_matrix() {
    for (case_idx, words) in packed_u32_cases().iter().enumerate() {
        let expected = oracle_fnv1a32_packed_low8(words);
        assert_eq!(
            fnv1a32_packed_u32_low8(words),
            expected,
            "Fix: fnv1a32_packed_u32_low8 adversarial case {case_idx} len={} must match the independent oracle.",
            words.len()
        );
        let bytes: Vec<u8> = words.iter().map(|word| (*word & 0xFF) as u8).collect();
        assert_eq!(
            fnv1a32(&bytes),
            expected,
            "Fix: fnv1a32 byte path adversarial case {case_idx} must match packed-low8 oracle."
        );
    }
}

fn oracle_pair_reduce(chunks: &[Crc32Chunk]) -> Vec<Crc32Chunk> {
    let mut reduced = Vec::with_capacity(chunks.len().div_ceil(2));
    for pair in chunks.chunks(2) {
        let chunk = match pair {
            [left, right] => Crc32Chunk {
                len: left
                    .len
                    .checked_add(right.len)
                    .expect("oracle chunk length overflow"),
                crc: oracle_crc32_combine(left.crc, right.crc, right.len),
            },
            [tail] => *tail,
            [] => continue,
            _ => unreachable!("chunks of two"),
        };
        reduced.push(chunk);
    }
    reduced
}

fn oracle_crc32_combine(left_crc: u32, right_crc: u32, right_len: u64) -> u32 {
    if right_len == 0 {
        return left_crc;
    }
    let mut odd = [0u32; 32];
    let mut even = [0u32; 32];
    odd[0] = CRC32_POLY;
    let mut row = 1u32;
    for slot in odd.iter_mut().skip(1) {
        *slot = row;
        row <<= 1;
    }
    gf2_matrix_square(&mut even, &odd);
    gf2_matrix_square(&mut odd, &even);

    let mut len = right_len;
    let mut crc = left_crc;
    loop {
        gf2_matrix_square(&mut even, &odd);
        if (len & 1) != 0 {
            crc = gf2_matrix_times(&even, crc);
        }
        len >>= 1;
        if len == 0 {
            break;
        }
        gf2_matrix_square(&mut odd, &even);
        if (len & 1) != 0 {
            crc = gf2_matrix_times(&odd, crc);
        }
        len >>= 1;
        if len == 0 {
            break;
        }
    }
    crc ^ right_crc
}

fn gf2_matrix_times(matrix: &[u32; 32], mut vector: u32) -> u32 {
    let mut sum = 0u32;
    let mut index = 0usize;
    while vector != 0 {
        if (vector & 1) != 0 {
            sum ^= matrix[index];
        }
        vector >>= 1;
        index += 1;
    }
    sum
}

fn gf2_matrix_square(square: &mut [u32; 32], matrix: &[u32; 32]) {
    for index in 0..32 {
        square[index] = gf2_matrix_times(matrix, matrix[index]);
    }
}

fn oracle_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ CRC32_POLY
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    table
}

fn oracle_crc32(bytes: &[u8]) -> u32 {
    let table = oracle_crc32_table();
    let mut crc = CRC32_INIT;
    for &byte in bytes {
        let idx = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ table[idx];
    }
    crc ^ CRC32_INIT
}

fn oracle_fnv1a32_packed_low8(words: &[u32]) -> u32 {
    let mut hash = FNV1A32_OFFSET;
    for &word in words {
        hash = (hash ^ (word & 0xFF)).wrapping_mul(FNV1A32_PRIME);
    }
    hash
}

fn byte_cases() -> Vec<Vec<u8>> {
    let mut cases = Vec::new();
    let lengths = [
        0usize, 1, 2, 3, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024,
    ];
    let fills = [0u8, 1, 0xFF, 0x7F, 0x80, 0x61, 0x00];

    for len in lengths {
        for fill in fills {
            cases.push(vec![fill; len]);
        }
        cases.push((0..len).map(|idx| idx as u8).collect());
        cases.push(
            (0..len)
                .map(|idx| idx.wrapping_mul(41).wrapping_add(3) as u8)
                .collect(),
        );
    }

    for seed in [0x01, 0x11, 0xBE, 0xEF, 0x80, 0xFE] {
        for len in lengths {
            cases.push(lcg_bytes(seed, len));
        }
    }

    for case in 0..16384usize {
        let len = case % 513;
        cases.push(lcg_bytes(case as u8 ^ 0xA5, len));
    }

    cases
}

fn packed_u32_cases() -> Vec<Vec<u32>> {
    let mut cases = Vec::new();
    let lengths = [
        0usize, 1, 2, 3, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024,
    ];
    let fills = [0u32, 1, 0xFF, 0x61, 0xFFFF_FF61, 0xCAFE_00BE, u32::MAX];

    for len in lengths {
        for fill in fills {
            cases.push(vec![fill; len]);
        }
        cases.push((0..len).map(|idx| idx as u32).collect());
        cases.push((0..len).map(|idx| (idx as u32) << 24).collect());
    }

    for seed in [0x01, 0x11, 0xBE, 0xEF, 0x80, 0xFE] {
        for len in lengths {
            cases.push(lcg_words(seed, len));
        }
    }

    for case in 0..16384usize {
        let len = case % 513;
        cases.push(lcg_words(case as u32 ^ 0x5A5A_5A5A, len));
    }

    cases
}

fn lcg_bytes(seed: u8, len: usize) -> Vec<u8> {
    let mut state = u32::from(seed);
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx % 13) as u32);
            (state ^ (idx as u32).wrapping_mul(0x85EB_CA6B)) as u8
        })
        .collect()
}

fn lcg_words(seed: u32, len: usize) -> Vec<u32> {
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

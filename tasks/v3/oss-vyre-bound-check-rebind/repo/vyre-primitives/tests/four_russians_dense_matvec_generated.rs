//! Generated coverage for dense byte-tile Four Russians boolean matvec.
//!
//! This primitive is the packed graph/dataflow building block behind dense
//! frontier waves: eight source-column tests collapse into one LUT load per
//! destination word, then tiles are OR-reduced across the active frontier.

mod common;
use common::u32_bytes;
use vyre_primitives::bitset::four_russians::{
    dense_matvec_byte_lut, dense_matvec_byte_lut_words, dense_matvec_cpu_ref,
    four_russians_dense_matvec_byte_lut, frontier_words_for_byte_tiles, BYTE_TILE_STATES,
    BYTE_TILE_WIDTH, DENSE_MATVEC_OP_ID,
};
use vyre_reference::value::Value;

#[test]
fn generated_dense_matvec_luts_match_naive_boolean_semantics() {
    let mut checked = 0usize;

    for tile_count in 0..=18u32 {
        for dst_words in 1..=5u32 {
            for seed in 0..64u32 {
                let columns = generated_columns(tile_count, dst_words, seed);
                let frontier = generated_frontier(tile_count, seed.rotate_left(7));
                let lut = dense_matvec_byte_lut(&columns, tile_count, dst_words);

                assert_eq!(
                    lut.len() as u32,
                    dense_matvec_byte_lut_words(tile_count, dst_words),
                    "Fix: LUT word-count helper drifted for tile_count={tile_count}, dst_words={dst_words}."
                );
                assert_eq!(
                    dense_matvec_cpu_ref(&frontier, &lut, tile_count, dst_words),
                    naive_dense_matvec(&frontier, &columns, tile_count, dst_words),
                    "Fix: dense Four-Russians matvec drifted for tile_count={tile_count}, dst_words={dst_words}, seed={seed}."
                );
                checked += 1;
            }
        }
    }

    assert!(
        checked >= 6_000,
        "Fix: generated dense Four-Russians matrix must cover thousands of cases; got {checked}."
    );
}

#[test]
fn generated_dense_matvec_frontier_bytes_select_expected_lut_rows() {
    for tile_count in 1..=32u32 {
        let dst_words = 3u32;
        let columns = generated_columns(tile_count, dst_words, tile_count ^ 0xA5A5);
        let lut = dense_matvec_byte_lut(&columns, tile_count, dst_words);

        for tile in 0..tile_count {
            for active_byte in [0u32, 1, 2, 3, 7, 31, 127, 255] {
                let mut frontier = vec![0u32; frontier_words_for_byte_tiles(tile_count) as usize];
                frontier[(tile / 4) as usize] = active_byte << ((tile % 4) * 8);
                let actual = dense_matvec_cpu_ref(&frontier, &lut, tile_count, dst_words);
                let expected = naive_dense_matvec(&frontier, &columns, tile_count, dst_words);

                assert_eq!(
                    actual, expected,
                    "Fix: tile {tile} active_byte={active_byte:#04x} selected the wrong precomputed row."
                );
            }
        }
    }
}

#[test]
fn dense_matvec_ir_overwrites_dirty_output_words() {
    let tile_count = 3u32;
    let dst_words = 2u32;
    let columns = generated_columns(tile_count, dst_words, 0xC001_CAFE);
    let frontier = generated_frontier(tile_count, 0xFACE_FEED);
    let lut = dense_matvec_byte_lut(&columns, tile_count, dst_words);
    let program =
        four_russians_dense_matvec_byte_lut("frontier", "tile_lut", "out", tile_count, dst_words);
    let expected = dense_matvec_cpu_ref(&frontier, &lut, tile_count, dst_words);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&frontier)),
            Value::from(u32_bytes(&lut)),
            Value::from(u32_bytes(&[u32::MAX, u32::MAX])),
        ],
    )
    .expect("Fix: dense Four-Russians matvec Program must execute in the reference oracle.");

    assert_eq!(
        outputs[0].to_bytes(),
        u32_bytes(&expected),
        "Fix: dense Four-Russians matvec Program must overwrite dirty output with LUT-reduced boolean matvec result."
    );
}

#[test]
fn dense_matvec_source_contracts_stay_gpu_oriented() {
    let source = include_str!("../src/bitset/four_russians.rs");
    for required in [
        DENSE_MATVEC_OP_ID,
        "frontier_words_for_byte_tiles",
        "dense_matvec_byte_lut",
        "four_russians_dense_matvec_byte_lut",
        "dense_matvec_cpu_ref",
        "Expr::rem",
        "Expr::shr",
        "Expr::bitor",
    ] {
        assert!(
            source.contains(required),
            "Fix: dense Four-Russians primitive must retain required implementation marker `{required}`."
        );
    }
}

fn generated_columns(tile_count: u32, dst_words: u32, seed: u32) -> Vec<u32> {
    let len = tile_count as usize * BYTE_TILE_WIDTH as usize * dst_words as usize;
    (0..len)
        .map(|idx| mix(seed ^ (idx as u32).wrapping_mul(0x9E37_79B9)))
        .collect()
}

fn generated_frontier(tile_count: u32, seed: u32) -> Vec<u32> {
    let len = frontier_words_for_byte_tiles(tile_count) as usize;
    (0..len)
        .map(|idx| {
            let mut word = mix(seed ^ idx as u32);
            let used_tiles_in_word = tile_count.saturating_sub((idx as u32) * 4).min(4);
            if used_tiles_in_word < 4 {
                let used_bits = used_tiles_in_word * 8;
                let mask = if used_bits == 0 {
                    0
                } else {
                    u32::MAX >> (32 - used_bits)
                };
                word &= mask;
            }
            word
        })
        .collect()
}

fn naive_dense_matvec(
    frontier: &[u32],
    columns: &[u32],
    tile_count: u32,
    dst_words: u32,
) -> Vec<u32> {
    let mut out = vec![0u32; dst_words as usize];
    for tile in 0..tile_count {
        let active_byte = if frontier.is_empty() {
            0
        } else {
            (frontier[(tile / 4) as usize] >> ((tile % 4) * 8)) & (BYTE_TILE_STATES - 1)
        };
        for source_bit in 0..BYTE_TILE_WIDTH {
            if (active_byte & (1 << source_bit)) == 0 {
                continue;
            }
            for dst_word in 0..dst_words {
                let column_idx =
                    ((tile * BYTE_TILE_WIDTH + source_bit) * dst_words + dst_word) as usize;
                out[dst_word as usize] |= columns[column_idx];
            }
        }
    }
    out
}

fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

//! Generated integration coverage for self-substrate dense bitset transforms.
//!
//! The primitive-level dense Four-Russians matvec is only useful if the
//! self-substrate exposes it as a reusable transform for graph/dataflow
//! schedulers. This test keeps that wiring broad and deterministic.

mod common;
use common::u32_bytes;
use vyre_reference::value::Value;
use vyre_self_substrate::data::bitset_transform_pipeline::{
    dense_boolean_matvec_lut, dense_matvec_frontier_words, dense_matvec_lut_words,
    four_russians_dense_matvec_program,
};

const BYTE_TILE_WIDTH: u32 = 8;

#[test]
fn generated_dense_matvec_pipeline_matches_naive_semantics() {
    let mut checked = 0usize;

    for tile_count in 0..=24u32 {
        for dst_words in 1..=4u32 {
            for seed in 0..64u32 {
                let columns = generated_columns(tile_count, dst_words, seed ^ 0x1357_2468);
                let frontier = generated_frontier(tile_count, seed ^ 0xACE0_BDF1);
                let lut = dense_boolean_matvec_lut(&columns, tile_count, dst_words);

                assert_eq!(
                    dense_matvec_frontier_words(tile_count) as usize,
                    frontier.len(),
                    "Fix: self-substrate frontier sizing drifted for tile_count={tile_count}."
                );
                assert_eq!(
                    dense_matvec_lut_words(tile_count, dst_words) as usize,
                    lut.len(),
                    "Fix: self-substrate LUT sizing drifted for tile_count={tile_count}, dst_words={dst_words}."
                );
                assert_eq!(
                    vyre_primitives::bitset::four_russians::dense_matvec_cpu_ref(
                        &frontier, &lut, tile_count, dst_words,
                    ),
                    naive_dense_matvec(&frontier, &columns, tile_count, dst_words),
                    "Fix: self-substrate dense matvec transform drifted for tile_count={tile_count}, dst_words={dst_words}, seed={seed}."
                );
                checked += 1;
            }
        }
    }

    assert!(
        checked >= 6_000,
        "Fix: self-substrate dense matvec generated coverage must stay broad; got {checked}."
    );
}

#[test]
fn dense_matvec_pipeline_program_overwrites_dirty_output() {
    let tile_count = 5u32;
    let dst_words = 3u32;
    let columns = generated_columns(tile_count, dst_words, 0xC0DE_5EED);
    let frontier = generated_frontier(tile_count, 0xF00D_FACE);
    let lut = dense_boolean_matvec_lut(&columns, tile_count, dst_words);
    let expected = naive_dense_matvec(&frontier, &columns, tile_count, dst_words);
    let program =
        four_russians_dense_matvec_program("frontier", "tile_lut", "out", tile_count, dst_words);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&frontier)),
            Value::from(u32_bytes(&lut)),
            Value::from(u32_bytes(&vec![u32::MAX; dst_words as usize])),
        ],
    )
    .expect("Fix: self-substrate dense matvec Program must execute in reference oracle.");

    assert_eq!(
        outputs[0].to_bytes(),
        u32_bytes(&expected),
        "Fix: dense matvec transform must overwrite dirty output with the exact boolean-semiring result."
    );
}

#[test]
fn dense_matvec_pipeline_source_keeps_primitive_wiring() {
    let source = include_str!("../src/data/bitset_transform_pipeline.rs");
    for required in [
        "dense_boolean_matvec_lut",
        "four_russians_dense_matvec_program",
        "reference_dense_boolean_matvec",
        "four_russians_dense_matvec_byte_lut",
        "dense_matvec_byte_lut",
        "DENSE_MATVEC_OP_ID",
    ] {
        assert!(
            source.contains(required),
            "Fix: self-substrate bitset pipeline must keep dense Four-Russians wiring marker `{required}`."
        );
    }
}

fn generated_columns(tile_count: u32, dst_words: u32, seed: u32) -> Vec<u32> {
    let len = tile_count as usize * BYTE_TILE_WIDTH as usize * dst_words as usize;
    (0..len)
        .map(|idx| mix(seed ^ (idx as u32).wrapping_mul(0x45D9_F3B)))
        .collect()
}

fn generated_frontier(tile_count: u32, seed: u32) -> Vec<u32> {
    let len = dense_matvec_frontier_words(tile_count) as usize;
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
            (frontier[(tile / 4) as usize] >> ((tile % 4) * 8)) & 0xFF
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

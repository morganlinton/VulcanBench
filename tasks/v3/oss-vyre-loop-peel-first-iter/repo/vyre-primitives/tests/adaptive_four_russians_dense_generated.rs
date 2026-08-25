//! Generated coverage for graph-level Four-Russians dense traversal.
//!
//! Dense adaptive traversal previously had only row-scan bitmatrix execution.
//! These tests keep the new source-column byte-tile path equivalent to that
//! oracle across generated graph shapes and frontier densities.
#![cfg(all(feature = "graph", feature = "cpu-parity"))]

mod common;
use common::u32_bytes;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::adaptive_traverse::{
    adaptive_four_russians_dense_step, cpu_dense_step, cpu_four_russians_dense_step,
    four_russians_dense_columns_from_adj_rows, four_russians_dense_lut_from_adj_rows,
    four_russians_dense_lut_words, four_russians_frontier_words, four_russians_source_tile_count,
    select_dense_traversal_kernel, DenseTraversalKernel, DENSE_THRESHOLD_PCT,
    FOUR_RUSSIANS_DENSE_OP_ID,
};
use vyre_reference::value::Value;

#[test]
fn generated_four_russians_dense_matches_row_scan_dense() {
    let mut checked = 0usize;

    for seed in 0..8_192u32 {
        let node_count = 1 + seed % 129;
        let adj = generated_dense_reverse_rows(seed, node_count);
        let frontier = generated_frontier(seed.rotate_left(9), node_count);
        let row_scan = cpu_dense_step(&frontier, &adj, node_count);
        let four_russians = cpu_four_russians_dense_step(&frontier, &adj, node_count)
            .expect("Fix: generated dense rows must transpose into Four-Russians columns.");

        assert_eq!(
            four_russians, row_scan,
            "Fix: Four-Russians dense traversal diverged from row-scan dense traversal at seed={seed}, node_count={node_count}."
        );
        checked += 1;
    }

    assert_eq!(checked, 8_192);
}

#[test]
fn generated_four_russians_lut_shapes_are_exact() {
    for node_count in 1..=512u32 {
        let adj = generated_dense_reverse_rows(node_count ^ 0xA5A5_5A5A, node_count);
        let columns = four_russians_dense_columns_from_adj_rows(node_count, &adj)
            .expect("Fix: valid dense rows must transpose.");
        let lut = four_russians_dense_lut_from_adj_rows(node_count, &adj)
            .expect("Fix: valid dense rows must build a LUT.");
        let words = bitset_words(node_count);
        let tile_count = four_russians_source_tile_count(node_count);

        assert_eq!(
            columns.len(),
            (tile_count * 8 * words) as usize,
            "Fix: column table shape drifted for node_count={node_count}."
        );
        assert_eq!(
            lut.len() as u32,
            four_russians_dense_lut_words(node_count),
            "Fix: dense LUT shape helper drifted for node_count={node_count}."
        );
        assert_eq!(
            four_russians_frontier_words(node_count),
            tile_count.div_ceil(4),
            "Fix: Four-Russians frontier word helper drifted for node_count={node_count}."
        );
    }
}

#[test]
fn four_russians_dense_program_matches_row_scan_on_reference_oracle() {
    let node_count = 73u32;
    let adj = generated_dense_reverse_rows(0x5151_5151, node_count);
    let frontier = generated_frontier(0x1234_9876, node_count);
    let lut = four_russians_dense_lut_from_adj_rows(node_count, &adj)
        .expect("Fix: valid dense rows must build a LUT.");
    let expected = cpu_dense_step(&frontier, &adj, node_count);
    let program = adaptive_four_russians_dense_step("frontier", "tile_lut", "out", node_count);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&frontier)),
            Value::from(u32_bytes(&lut)),
            Value::from(u32_bytes(&vec![
                u32::MAX;
                bitset_words(node_count) as usize
            ])),
        ],
    )
    .expect("Fix: graph-level Four-Russians dense Program must execute.");

    assert_eq!(
        outputs[0].to_bytes(),
        u32_bytes(&expected),
        "Fix: graph-level Four-Russians dense Program must overwrite dirty output with row-scan-equivalent traversal result."
    );
}

#[test]
fn dense_kernel_selector_prefers_four_russians_only_when_reusable_and_dense() {
    assert_eq!(
        select_dense_traversal_kernel(128, 64, 4),
        DenseTraversalKernel::FourRussiansByteTile
    );
    assert_eq!(
        select_dense_traversal_kernel(128, 1, 4),
        DenseTraversalKernel::RowScanBitmatrix
    );
    assert_eq!(
        select_dense_traversal_kernel(128, 64, 1),
        DenseTraversalKernel::RowScanBitmatrix
    );
    assert_eq!(
        select_dense_traversal_kernel(32, 32, 4),
        DenseTraversalKernel::RowScanBitmatrix
    );
}

#[test]
fn adaptive_four_russians_source_contracts_stay_wired_to_packed_matvec() {
    let source = include_str!("../src/graph/adaptive_traverse.rs");
    for required in [
        FOUR_RUSSIANS_DENSE_OP_ID,
        "four_russians_dense_columns_from_adj_rows",
        "four_russians_dense_lut_from_adj_rows",
        "adaptive_four_russians_dense_step",
        "cpu_four_russians_dense_step",
        "four_russians_dense_matvec_byte_lut",
        "dense_matvec_byte_lut",
    ] {
        assert!(
            source.contains(required),
            "Fix: adaptive traversal must keep Four-Russians dense marker `{required}`."
        );
    }
}

fn generated_dense_reverse_rows(seed: u32, node_count: u32) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut rows = vec![0u32; node_count as usize * words];
    let mut state = seed ^ 0xB17B_0015;

    for dst in 0..node_count {
        let degree = 1 + (next(&mut state) % 8);
        for edge_index in 0..degree {
            let src = next(&mut state).wrapping_add(dst.rotate_left(edge_index % 31)) % node_count;
            rows[dst as usize * words + src as usize / 32] |= 1u32 << (src % 32);
        }
    }

    rows
}

fn generated_frontier(seed: u32, node_count: u32) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut state = seed ^ 0xF0F0_1357;
    let mut frontier = vec![0u32; words];
    for node in 0..node_count {
        let dense_bias = if node_count >= 64 {
            DENSE_THRESHOLD_PCT / 5
        } else {
            1
        };
        if next(&mut state) % 8 <= dense_bias {
            frontier[node as usize / 32] |= 1u32 << (node % 32);
        }
    }
    if frontier.iter().all(|&word| word == 0) {
        let node = seed % node_count;
        frontier[node as usize / 32] |= 1u32 << (node % 32);
    }
    if node_count % 32 != 0 {
        let valid_bits = node_count % 32;
        let mask = (1u32 << valid_bits) - 1;
        let last = frontier.len() - 1;
        frontier[last] &= mask;
    }
    frontier
}

fn next(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

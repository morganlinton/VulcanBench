//! Parity test: GPU csr_backward_or_changed reaches source lanes across blocks.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::graph::csr_backward_or_changed::{
    csr_backward_or_changed_parallel, csr_backward_or_changed_parallel_grid,
};
use vyre_primitives::graph::program_graph::ProgramGraphShape;

fn set_bit(words: &mut [u32], node: u32) {
    words[(node / 32) as usize] |= 1 << (node & 31);
}

fn run_once(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> (Vec<u32>, u32) {
    let words = ((node_count + 31) / 32).max(1) as usize;
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = csr_backward_or_changed_parallel(
        ProgramGraphShape::new(node_count, edge_targets.len().max(1) as u32),
        "frontier",
        "changed",
        allow_mask,
    );
    let inputs = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(frontier),
        vec![0u8; 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_backward_or_changed_parallel_grid(node_count));
    let outputs = with_live_backend("CSR backward-or-changed primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA CSR backward-or-changed dispatch failed: {error}")
            })
    });
    let mut frontier_out = bytes_u32(&outputs[0]);
    frontier_out.truncate(words);
    let changed = bytes_u32(&outputs[1])[0];
    (frontier_out, changed)
}

#[test]
fn cuda_backward_or_changed_reaches_source_past_first_block() {
    let node_count = 513;
    let words = ((node_count + 31) / 32).max(1) as usize;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let targets = vec![512u32];
    let masks = vec![1u32];
    let mut frontier = vec![0u32; words];
    set_bit(&mut frontier, 512);

    let (gpu, changed) = run_once(
        node_count,
        &offsets,
        &targets,
        &masks,
        &frontier,
        0xFFFF_FFFF,
    );
    let mut expected = vec![0u32; words];
    set_bit(&mut expected, 300);
    set_bit(&mut expected, 512);
    assert_eq!(gpu, expected);
    assert_eq!(changed, 1);
}

#[test]
fn cuda_backward_or_changed_respects_edge_mask_without_false_change() {
    let node_count = 513;
    let words = ((node_count + 31) / 32).max(1) as usize;
    let mut offsets = vec![0u32; node_count as usize + 1];
    for offset in offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let targets = vec![512u32];
    let masks = vec![0b10u32];
    let mut frontier = vec![0u32; words];
    set_bit(&mut frontier, 512);

    let (gpu, changed) = run_once(node_count, &offsets, &targets, &masks, &frontier, 0b01);
    assert_eq!(gpu, frontier);
    assert_eq!(changed, 0);
}

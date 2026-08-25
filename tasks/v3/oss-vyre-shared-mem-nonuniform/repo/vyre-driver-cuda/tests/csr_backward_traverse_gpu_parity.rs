//! Parity test: GPU csr_backward_traverse one-step matches CPU oracle.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::graph::csr_backward_traverse::{
    cpu_ref, csr_backward_traverse, csr_backward_traverse_dispatch_grid,
};
use vyre_primitives::graph::program_graph::ProgramGraphShape;

fn run(
    node_count: u32,
    edge_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = ((node_count + 31) / 32).max(1);
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = csr_backward_traverse(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
        allow_mask,
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(frontier),
        // frontier_out: zero-init.
        vec![0u8; words as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(csr_backward_traverse_dispatch_grid(node_count));
    let outputs = with_live_backend("CSR backward traverse", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA CSR backward traverse dispatch failed: {error}")
            })
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

#[test]
fn cuda_csr_backward_chain_one_step() {
    // Forward CFG 0 -> 1 -> 2 -> 3.
    let edge_offsets = vec![0u32, 1, 2, 3, 3];
    let edge_targets = vec![1u32, 2, 3];
    let edge_kind_mask = vec![1u32; 3];
    // frontier_in = {3}. Backward step → {2} (only src that points to 3).
    let frontier = vec![0b1000u32];
    let cpu = cpu_ref(
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    let gpu = run(
        4,
        3,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0b0100u32]);
}

#[test]
fn cuda_csr_backward_diamond_one_step() {
    // Forward 0 -> {1, 2} -> 3.
    let edge_offsets = vec![0u32, 2, 3, 4, 4];
    let edge_targets = vec![1u32, 2, 3, 3];
    let edge_kind_mask = vec![1u32; 4];
    // frontier_in = {3}. Backward → {1, 2}.
    let frontier = vec![0b1000u32];
    let cpu = cpu_ref(
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    let gpu = run(
        4,
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0b0110u32]);
}

#[test]
fn cuda_csr_backward_kind_mask_filters() {
    let edge_offsets = vec![0u32, 1, 1];
    let edge_targets = vec![1u32];
    let edge_kind_mask = vec![0b0010u32]; // kind bit 1
    let frontier = vec![0b10u32];
    // allow=0b0001 (kind 0)  -  edge filtered out, no backward step.
    let cpu = cpu_ref(
        2,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0b0001,
    );
    let gpu = run(
        2,
        1,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0b0001,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32]);
}

#[test]
fn cuda_csr_backward_empty_frontier() {
    let edge_offsets = vec![0u32, 1, 2, 3, 3];
    let edge_targets = vec![1u32, 2, 3];
    let edge_kind_mask = vec![1u32; 3];
    let frontier = vec![0u32];
    let cpu = cpu_ref(
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    let gpu = run(
        4,
        3,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32]);
}

#[test]
fn cuda_csr_backward_reaches_source_past_first_workgroup() {
    let node_count = 513u32;
    let words = node_count.div_ceil(32) as usize;
    let mut edge_offsets = vec![0u32; node_count as usize + 1];
    for offset in edge_offsets.iter_mut().skip(301) {
        *offset = 1;
    }
    let edge_targets = vec![512u32];
    let edge_kind_mask = vec![1u32];
    let mut frontier = vec![0u32; words];
    frontier[512 / 32] |= 1u32 << (512 % 32);

    let cpu = cpu_ref(
        node_count,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );
    let gpu = run(
        node_count,
        1,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
    );

    let mut expected = vec![0u32; words];
    expected[300 / 32] |= 1u32 << (300 % 32);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, expected);
}

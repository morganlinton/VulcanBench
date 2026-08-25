//! Parity test: vyre-primitives persistent_bfs Program matches CPU oracle.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_foundation::ir::BufferAccess;
use vyre_primitives::graph::persistent_bfs::{
    cpu_ref, persistent_bfs, persistent_bfs_batch, persistent_bfs_batch_dispatch_grid,
    persistent_bfs_single_dispatch_grid,
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
    max_iters: u32,
) -> (Vec<u32>, u32) {
    let words = ((node_count + 31) / 32).max(1);
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = persistent_bfs(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
        allow_mask,
        max_iters,
    );
    let inputs: Vec<Vec<u8>> = program
        .buffers()
        .iter()
        .filter(|buffer| buffer.access() != BufferAccess::Workgroup)
        .map(|buffer| {
            let declared_words = buffer.count().max(1) as usize;
            match buffer.name() {
                "pg_nodes" => u32_bytes(&pg_nodes),
                "pg_edge_offsets" => u32_bytes(edge_offsets),
                "pg_edge_targets" => u32_bytes(edge_targets),
                "pg_edge_kind_mask" => u32_bytes(edge_kind_mask),
                "pg_node_tags" => u32_bytes(&pg_node_tags),
                "frontier_in" => u32_bytes(frontier),
                "frontier_out" | "changed" => vec![0u8; declared_words * 4],
                other => panic!("Unexpected buffer in persistent_bfs program: {other}"),
            }
        })
        .collect();
    let mut config = DispatchConfig::default();
    config.grid_override = Some(persistent_bfs_single_dispatch_grid(node_count));
    let outputs = with_live_backend("persistent BFS primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA persistent BFS primitive dispatch failed: {error}")
            })
    });
    let mut frontier_out = bytes_u32(&outputs[0]);
    frontier_out.truncate(words as usize);
    let changed = bytes_u32(&outputs[1])[0];
    (frontier_out, changed)
}

fn run_batch(
    node_count: u32,
    edge_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontiers: &[u32],
    query_count: u32,
    allow_mask: u32,
    max_iters: u32,
) -> (Vec<u32>, Vec<u32>) {
    let words = ((node_count + 31) / 32).max(1);
    let total_words = words as usize * query_count.max(1) as usize;
    let pg_nodes = vec![0u32; node_count as usize];
    let pg_node_tags = vec![0u32; node_count as usize];
    let program = persistent_bfs_batch(
        ProgramGraphShape::new(node_count, edge_count.max(1)),
        "frontier_in",
        "frontier_out",
        "changed",
        query_count,
        allow_mask,
        max_iters,
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(&pg_nodes),
        u32_bytes(edge_offsets),
        u32_bytes(edge_targets),
        u32_bytes(edge_kind_mask),
        u32_bytes(&pg_node_tags),
        u32_bytes(frontiers),
        vec![0u8; total_words * 4],
        vec![0u8; query_count.max(1) as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(persistent_bfs_batch_dispatch_grid(node_count, query_count));
    let outputs = with_live_backend("persistent BFS primitive batch", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA persistent BFS primitive batch dispatch failed: {error}")
            })
    });
    let mut frontier_out = bytes_u32(&outputs[0]);
    frontier_out.truncate(total_words);
    let mut changed = bytes_u32(&outputs[1]);
    changed.truncate(query_count as usize);
    (frontier_out, changed)
}

fn cpu_ref_batch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontiers: &[u32],
    query_count: u32,
    allow_mask: u32,
    max_iters: u32,
) -> (Vec<u32>, Vec<u32>) {
    let words = ((node_count + 31) / 32).max(1) as usize;
    let mut frontier_out = Vec::with_capacity(words * query_count as usize);
    let mut changed_out = Vec::with_capacity(query_count as usize);
    for query in 0..query_count as usize {
        let start = query * words;
        let end = start + words;
        let (frontier, changed) = cpu_ref(
            node_count,
            edge_offsets,
            edge_targets,
            edge_kind_mask,
            &frontiers[start..end],
            allow_mask,
            max_iters,
        );
        frontier_out.extend_from_slice(&frontier);
        changed_out.push(changed);
    }
    (frontier_out, changed_out)
}

#[test]
fn cuda_persistent_bfs_chain_converges_changed_set() {
    let n = 4u32;
    let edge_offsets = vec![0u32, 1, 2, 3, 3];
    let edge_targets = vec![1u32, 2, 3];
    let edge_kind_mask = vec![1u32; 3];
    let frontier = vec![0b0001u32];
    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    let gpu = run(
        n,
        3,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0, vec![0b1111u32]);
    assert_eq!(gpu.1, 1);
}

#[test]
fn cuda_persistent_bfs_diamond_converges() {
    let n = 4u32;
    let edge_offsets = vec![0u32, 2, 3, 4, 4];
    let edge_targets = vec![1u32, 2, 3, 3];
    let edge_kind_mask = vec![1u32; 4];
    let frontier = vec![0b0001u32];
    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    let gpu = run(
        n,
        4,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0, vec![0b1111u32]);
}

#[test]
fn cuda_persistent_bfs_isolated_seed_unchanged() {
    let n = 3u32;
    let edge_offsets = vec![0u32, 0, 0, 0];
    let edge_targets: Vec<u32> = Vec::new();
    let edge_kind_mask: Vec<u32> = Vec::new();
    let padded_edge_targets = vec![0u32; 1];
    let padded_edge_kind_mask = vec![0u32; 1];
    let frontier = vec![0b001u32];
    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    let gpu = run(
        n,
        0,
        &edge_offsets,
        &padded_edge_targets,
        &padded_edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        8,
    );
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0, vec![0b001u32]);
    assert_eq!(gpu.1, 0);
}

#[test]
fn cuda_persistent_bfs_large_no_edges_converges_without_changed() {
    let n = 513u32;
    let edge_offsets = vec![0u32; n as usize + 1];
    let edge_targets: Vec<u32> = Vec::new();
    let edge_kind_mask: Vec<u32> = Vec::new();
    let padded_edge_targets = vec![0u32; 1];
    let padded_edge_kind_mask = vec![0u32; 1];
    let words = ((n + 31) / 32) as usize;
    let mut frontier = vec![0u32; words];
    frontier[8] = 1;

    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        64,
    );
    let gpu = run(
        n,
        0,
        &edge_offsets,
        &padded_edge_targets,
        &padded_edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        64,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0[8], 1);
    assert!(gpu.0[..8].iter().all(|word| *word == 0));
    assert!(gpu.0[9..].iter().all(|word| *word == 0));
    assert_eq!(gpu.1, 0);
}

#[test]
fn cuda_persistent_bfs_large_graph_crosses_workgroup_boundary() {
    let n = 513u32;
    let mut edge_offsets = vec![0u32; n as usize + 1];
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    for src in 0..n {
        edge_offsets[src as usize] = edge_targets.len() as u32;
        if src == 256 {
            edge_targets.push(512);
            edge_kind_mask.push(1);
        }
    }
    edge_offsets[n as usize] = edge_targets.len() as u32;
    let mut frontier = vec![0u32; ((n + 31) / 32) as usize];
    frontier[8] = 1;

    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        1,
    );
    let gpu = run(
        n,
        edge_targets.len() as u32,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        1,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0[8], 1);
    assert_eq!(gpu.0[16], 1);
    assert_eq!(gpu.1, 1);
}

#[test]
fn cuda_persistent_bfs_large_chain_honors_one_step_cap() {
    let n = 513u32;
    let mut edge_offsets = Vec::with_capacity(n as usize + 1);
    let mut edge_targets = Vec::with_capacity(n as usize - 1);
    let mut edge_kind_mask = Vec::with_capacity(n as usize - 1);
    edge_offsets.push(0);
    for src in 0..n {
        if src + 1 < n {
            edge_targets.push(src + 1);
            edge_kind_mask.push(1);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let mut frontier = vec![0u32; ((n + 31) / 32) as usize];
    frontier[0] = 1;

    let cpu = cpu_ref(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        1,
    );
    let gpu = run(
        n,
        edge_targets.len() as u32,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontier,
        0xFFFF_FFFF,
        1,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0[0], 0b11);
    assert!(
        gpu.0[1..].iter().all(|word| *word == 0),
        "Fix: one persistent-BFS iteration must not cascade past node 1 on a long chain."
    );
    assert_eq!(gpu.1, 1);
}

#[test]
fn cuda_persistent_bfs_batch_large_chain_honors_one_step_cap_per_query() {
    let n = 513u32;
    let words = ((n + 31) / 32) as usize;
    let query_count = 2u32;
    let mut edge_offsets = Vec::with_capacity(n as usize + 1);
    let mut edge_targets = Vec::with_capacity(n as usize - 1);
    let mut edge_kind_mask = Vec::with_capacity(n as usize - 1);
    edge_offsets.push(0);
    for src in 0..n {
        if src + 1 < n {
            edge_targets.push(src + 1);
            edge_kind_mask.push(1);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }
    let mut frontiers = vec![0u32; words * query_count as usize];
    frontiers[0] = 1;
    frontiers[words + 8] = 1;

    let cpu = cpu_ref_batch(
        n,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontiers,
        query_count,
        0xFFFF_FFFF,
        1,
    );
    let gpu = run_batch(
        n,
        edge_targets.len() as u32,
        &edge_offsets,
        &edge_targets,
        &edge_kind_mask,
        &frontiers,
        query_count,
        0xFFFF_FFFF,
        1,
    );

    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0[0], 0b11);
    assert!(gpu.0[1..words].iter().all(|word| *word == 0));
    assert_eq!(gpu.0[words + 8], 0b11);
    assert!(gpu.0[words..words + 8].iter().all(|word| *word == 0));
    assert!(gpu.0[words + 9..].iter().all(|word| *word == 0));
    assert_eq!(gpu.1, vec![1, 1]);
}

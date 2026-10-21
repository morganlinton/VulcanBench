//! Parity test: vyre-primitives functor_apply + matroid_exchange_bfs_step
//! match CPU oracles.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::graph::functorial::{functor_apply, functor_apply_cpu};
use vyre_primitives::graph::matroid::{matroid_exchange_bfs_step, matroid_exchange_bfs_step_cpu};

fn run_functor(source: &[u32], mapping: &[u32], target_size: u32) -> Vec<u32> {
    let n = source.len() as u32;
    let program = functor_apply("source", "mapping", "target", n);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(source),
        u32_bytes(mapping),
        // target: zero-init.
        vec![0u8; n as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((n + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("functor apply", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA functor-apply dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(target_size as usize);
    out
}

#[test]
fn cuda_functor_apply_identity() {
    let src = vec![10u32, 20, 30];
    let map = vec![0u32, 1, 2];
    let cpu = functor_apply_cpu(&src, &map, 3);
    let gpu = run_functor(&src, &map, 3);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, src);
}

#[test]
fn cuda_functor_apply_permutation() {
    let src = vec![10u32, 20, 30];
    let map = vec![2u32, 0, 1];
    let cpu = functor_apply_cpu(&src, &map, 3);
    let gpu = run_functor(&src, &map, 3);
    assert_eq!(gpu, cpu);
}

fn run_matroid_bfs(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: u32,
) -> (Vec<u32>, u32) {
    let program = matroid_exchange_bfs_step(
        "frontier_in",
        "exchange_adj",
        "visited",
        "frontier_out",
        "any_change",
        n,
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(frontier_in),
        u32_bytes(exchange_adj),
        u32_bytes(visited),
        // frontier_out: zero-init.
        vec![0u8; n as usize * 4],
        // any_change: zero-init.
        vec![0u8; 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((n + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("matroid exchange BFS", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA matroid BFS dispatch failed: {error}"))
    });
    let mut frontier_out = bytes_u32(&outputs[0]);
    frontier_out.truncate(n as usize);
    let any = bytes_u32(&outputs[1])[0];
    (frontier_out, any)
}

#[test]
fn cuda_matroid_bfs_one_step_advance() {
    let f = vec![1u32, 0, 0];
    let adj = vec![0u32, 1, 0, 0, 0, 0, 0, 0, 0];
    let v = vec![0u32; 3];
    let (cpu_out, cpu_any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
    let (gpu_out, gpu_any) = run_matroid_bfs(&f, &adj, &v, 3);
    assert_eq!(gpu_out, cpu_out);
    assert_eq!(gpu_any, if cpu_any { 1 } else { 0 });
    assert_eq!(gpu_out, vec![0, 1, 0]);
}

#[test]
fn cuda_matroid_bfs_visited_blocks_advance() {
    let f = vec![1u32, 0, 0];
    let adj = vec![0u32, 1, 0, 0, 0, 0, 0, 0, 0];
    let v = vec![0u32, 1, 0];
    let (cpu_out, cpu_any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
    let (gpu_out, gpu_any) = run_matroid_bfs(&f, &adj, &v, 3);
    assert_eq!(gpu_out, cpu_out);
    assert_eq!(gpu_any, if cpu_any { 1 } else { 0 });
    assert_eq!(gpu_out, vec![0, 0, 0]);
    assert_eq!(gpu_any, 0);
}

#[test]
fn cuda_matroid_bfs_empty_frontier_no_change() {
    let f = vec![0u32; 3];
    let adj = vec![1u32; 9];
    let v = vec![0u32; 3];
    let (cpu_out, cpu_any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
    let (gpu_out, gpu_any) = run_matroid_bfs(&f, &adj, &v, 3);
    assert_eq!(gpu_out, cpu_out);
    assert_eq!(gpu_any, if cpu_any { 1 } else { 0 });
    assert_eq!(gpu_any, 0);
}

#[test]
fn cuda_matroid_bfs_multi_source_advance() {
    // 4 nodes, frontier {0, 1}, edges 0→2 and 1→3.
    let f = vec![1u32, 1, 0, 0];
    let adj = vec![
        0u32, 0, 1, 0, // 0→2
        0, 0, 0, 1, // 1→3
        0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let v = vec![0u32; 4];
    let (cpu_out, cpu_any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 4);
    let (gpu_out, gpu_any) = run_matroid_bfs(&f, &adj, &v, 4);
    assert_eq!(gpu_out, cpu_out);
    assert_eq!(gpu_any, if cpu_any { 1 } else { 0 });
    assert_eq!(gpu_out, vec![0, 0, 1, 1]);
}

//! Parity test: GPU batched path reconstruction matches CPU oracle.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_primitives::graph::path_reconstruct::cpu_ref as path_reconstruct_cpu;
use vyre_self_substrate::path_reconstruct::{reconstruct_path_via, reconstruct_paths_via};

fn cpu_path(parent: &[u32], target: u32, max_depth: u32) -> (Vec<u32>, u32) {
    let mut scratch = Vec::new();
    let len = path_reconstruct_cpu(parent, target, max_depth, &mut scratch);
    (scratch, len)
}

#[test]
fn cuda_reconstruct_path_chain() {
    let parent = vec![0u32, 0, 1, 2];
    let mut gpu_scratch = Vec::new();
    let gpu_len = with_cuda_optimizer_dispatcher("path chain", |dispatcher| {
        reconstruct_path_via(dispatcher, &parent, 3, 4, &mut gpu_scratch).expect("dispatch")
    });
    let (cpu_scratch, cpu_len) = cpu_path(&parent, 3, 4);
    assert_eq!(gpu_len, cpu_len);
    assert_eq!(gpu_scratch, cpu_scratch);
}

#[test]
fn cuda_reconstruct_path_root_target() {
    let parent = vec![0u32, 0, 1];
    let mut gpu_scratch = Vec::new();
    let gpu_len = with_cuda_optimizer_dispatcher("path root", |dispatcher| {
        reconstruct_path_via(dispatcher, &parent, 0, 4, &mut gpu_scratch).expect("dispatch")
    });
    let (cpu_scratch, cpu_len) = cpu_path(&parent, 0, 4);
    assert_eq!(gpu_len, cpu_len);
    assert_eq!(gpu_scratch, cpu_scratch);
}

#[test]
fn cuda_reconstruct_path_cycle_caps_at_max_depth() {
    // Cycle 0 -> 1 -> 2 -> 0.
    let parent = vec![1u32, 2, 0];
    let mut gpu_scratch = Vec::new();
    let gpu_len = with_cuda_optimizer_dispatcher("path cycle", |dispatcher| {
        reconstruct_path_via(dispatcher, &parent, 0, 5, &mut gpu_scratch).expect("dispatch")
    });
    let (cpu_scratch, cpu_len) = cpu_path(&parent, 0, 5);
    assert_eq!(gpu_len, cpu_len);
    assert_eq!(gpu_scratch, cpu_scratch);
}

#[test]
fn cuda_reconstruct_paths_batched() {
    let parent = vec![0u32, 0, 1, 2, 3, 4];
    let targets = vec![5u32, 4, 3, 2, 1, 0];
    let max_depth = 6u32;
    let (paths, lens) = with_cuda_optimizer_dispatcher("batched paths", |dispatcher| {
        reconstruct_paths_via(dispatcher, &parent, &targets, max_depth).expect("dispatch")
    });
    assert_eq!(lens.len(), targets.len());
    for (i, &t) in targets.iter().enumerate() {
        let (cpu_scratch, cpu_len) = cpu_path(&parent, t, max_depth);
        let lo = i * max_depth as usize;
        let hi = lo + max_depth as usize;
        assert_eq!(lens[i], cpu_len, "len divergence at target {t}");
        assert_eq!(
            &paths[lo..hi],
            &cpu_scratch[..],
            "path divergence at target {t}"
        );
    }
}

#[test]
fn cuda_reconstruct_paths_oob_target_self_loops() {
    let parent = vec![0u32, 0, 1];
    // OOB target  -  cpu_ref reads parent.get(target).copied().unwrap_or(target) → self-loop.
    let targets = vec![100u32];
    let (paths, lens) = with_cuda_optimizer_dispatcher("oob target paths", |dispatcher| {
        reconstruct_paths_via(dispatcher, &parent, &targets, 4).expect("dispatch")
    });
    let (cpu_scratch, cpu_len) = cpu_path(&parent, 100, 4);
    assert_eq!(lens[0], cpu_len);
    assert_eq!(&paths[..4], &cpu_scratch[..]);
}

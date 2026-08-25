//! Parity tests for fixpoint::bitset_fixpoint, visual::packed_rgba_map,
//! and matching::region::dedup_regions_flag_program.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::fixpoint::bitset_fixpoint::bitset_fixpoint;
use vyre_primitives::matching::region::{
    dedup_regions_cluster_program, dedup_regions_flag_program, region_dedup_dispatch_grid,
};
use vyre_primitives::visual::packed_rgba_map::packed_rgba_map;

// ---------------------------------------------------------------------
// bitset_fixpoint: changed=1 iff current[w] != next[w] for any w.
// ---------------------------------------------------------------------

fn run_fixpoint(current: &[u32], next: &[u32]) -> u32 {
    assert_eq!(current.len(), next.len());
    let words = current.len() as u32;
    let program = bitset_fixpoint("current", "next", "changed", words);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(current), u32_bytes(next), vec![0u8; 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((words + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("bitset fixpoint", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA bitset-fixpoint dispatch failed: {error}"))
    });
    bytes_u32(&outputs[0])[0]
}

#[test]
fn cuda_bitset_fixpoint_no_change() {
    let v = vec![0xAAAAu32, 0u32, 0xFFFFu32];
    let gpu = run_fixpoint(&v, &v);
    assert_eq!(gpu, 0);
}

#[test]
fn cuda_bitset_fixpoint_one_word_changed() {
    let current = vec![0xAAAAu32, 0u32, 0xFFFFu32];
    let next = vec![0xAAAAu32, 1u32, 0xFFFFu32];
    let gpu = run_fixpoint(&current, &next);
    assert_eq!(gpu, 1);
}

#[test]
fn cuda_bitset_fixpoint_all_changed() {
    let current = vec![0u32; 8];
    let next = vec![0xFFFF_FFFFu32; 8];
    let gpu = run_fixpoint(&current, &next);
    assert_eq!(gpu, 1);
}

// ---------------------------------------------------------------------
// packed_rgba_map: identity copy.
// ---------------------------------------------------------------------

fn run_rgba_identity(pixels: &[u32]) -> Vec<u32> {
    let count = pixels.len() as u32;
    let program = packed_rgba_map("input", "output", count);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(pixels), vec![0u8; count as usize * 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("packed RGBA map", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA packed-RGBA dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(count as usize);
    out
}

#[test]
fn cuda_packed_rgba_map_identity_basic() {
    let pixels = vec![0xFF00_0000u32, 0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000];
    let gpu = run_rgba_identity(&pixels);
    assert_eq!(gpu, pixels);
}

#[test]
fn cuda_packed_rgba_map_identity_large() {
    let pixels: Vec<u32> = (0u32..512).map(|i| 0xFF00_0000 | i).collect();
    let gpu = run_rgba_identity(&pixels);
    assert_eq!(gpu, pixels);
}

// ---------------------------------------------------------------------
// dedup_regions_flag_program / dedup_regions_cluster_program: sorted span clusters.
// ---------------------------------------------------------------------

fn cpu_dedup_cluster_metadata(pids: &[u32], starts: &[u32], ends: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut survivors = vec![0u32; pids.len()];
    let mut merged_ends = ends.to_vec();
    for i in 0..pids.len() {
        let has_prev_overlap = (0..i).any(|j| pids[j] == pids[i] && ends[j] >= starts[i]);
        if has_prev_overlap {
            continue;
        }

        survivors[i] = 1;
        let mut merged_end = ends[i];
        for j in i + 1..pids.len() {
            if pids[j] != pids[i] || starts[j] > merged_end {
                break;
            }
            merged_end = merged_end.max(ends[j]);
        }
        merged_ends[i] = merged_end;
    }
    (survivors, merged_ends)
}

fn run_dedup_flag(pids: &[u32], starts: &[u32], ends: &[u32]) -> Vec<u32> {
    assert_eq!(pids.len(), starts.len());
    assert_eq!(pids.len(), ends.len());
    let count = pids.len() as u32;
    let program = dedup_regions_flag_program("pids", "starts", "ends", "survivors", count);
    // survivors is BufferAccess::WriteOnly so it does not consume an
    // input slot.
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(pids), u32_bytes(starts), u32_bytes(ends)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(region_dedup_dispatch_grid(count));
    let outputs = with_live_backend("dedup regions flag", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA dedup-regions flag dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(count as usize);
    out
}

fn run_dedup_cluster(pids: &[u32], starts: &[u32], ends: &[u32]) -> (Vec<u32>, Vec<u32>) {
    assert_eq!(pids.len(), starts.len());
    assert_eq!(pids.len(), ends.len());
    let count = pids.len() as u32;
    let program =
        dedup_regions_cluster_program("pids", "starts", "ends", "survivors", "merged_ends", count);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(pids), u32_bytes(starts), u32_bytes(ends)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(region_dedup_dispatch_grid(count));
    let outputs = with_live_backend("dedup regions cluster", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA dedup-regions cluster dispatch failed: {error}")
            })
    });
    let mut survivors = bytes_u32(&outputs[0]);
    survivors.truncate(count as usize);
    let mut merged_ends = bytes_u32(&outputs[1]);
    merged_ends.truncate(count as usize);
    (survivors, merged_ends)
}

#[test]
fn cuda_dedup_regions_flag_distinct_pids() {
    let pids = vec![1u32, 2, 3];
    let starts = vec![0u32, 10, 20];
    let ends = vec![5u32, 15, 25];
    let (cpu, _) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let gpu = run_dedup_flag(&pids, &starts, &ends);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 1, 1]); // all distinct.
}

#[test]
fn cuda_dedup_regions_flag_overlapping_drops_second() {
    let pids = vec![1u32, 1, 1];
    let starts = vec![0u32, 3, 100];
    let ends = vec![10u32, 12, 110];
    let (cpu, _) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let gpu = run_dedup_flag(&pids, &starts, &ends);
    assert_eq!(gpu, cpu);
    // Index 1 overlaps index 0 (start=3 <= end=10) → drop. Index 2
    // is past the prior end → keep.
    assert_eq!(gpu, vec![1, 0, 1]);
}

#[test]
fn cuda_dedup_regions_flag_pid_change_resets_overlap() {
    let pids = vec![1u32, 2, 2];
    let starts = vec![0u32, 5, 10];
    let ends = vec![20u32, 15, 25];
    let (cpu, _) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let gpu = run_dedup_flag(&pids, &starts, &ends);
    assert_eq!(gpu, cpu);
    // Index 1: pid changed → keep. Index 2: same pid, start=10 <=
    // prev end=15 → drop.
    assert_eq!(gpu, vec![1, 1, 0]);
}

#[test]
fn cuda_dedup_regions_flag_handles_nested_short_previous_span() {
    let pids = vec![7u32, 7, 7, 7];
    let starts = vec![0u32, 2, 9, 20];
    let ends = vec![10u32, 3, 12, 25];
    let (cpu, _) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let gpu = run_dedup_flag(&pids, &starts, &ends);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 0, 0, 1]);
}

#[test]
fn cuda_dedup_regions_cluster_outputs_merged_survivor_ends() {
    let pids = vec![7u32, 7, 7, 7, 8];
    let starts = vec![0u32, 2, 9, 20, 0];
    let ends = vec![10u32, 3, 12, 25, 4];
    let (cpu_flags, cpu_merged) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let (gpu_flags, gpu_merged) = run_dedup_cluster(&pids, &starts, &ends);

    assert_eq!(gpu_flags, cpu_flags);
    assert_eq!(gpu_merged, cpu_merged);
    assert_eq!(gpu_flags, vec![1, 0, 0, 1, 1]);
    assert_eq!(gpu_merged[0], 12);
    assert_eq!(gpu_merged[3], 25);
    assert_eq!(gpu_merged[4], 4);
}

#[test]
fn cuda_dedup_regions_cluster_covers_lanes_past_first_workgroup() {
    let count = 513usize;
    let pids = vec![3u32; count];
    let mut starts = Vec::with_capacity(count);
    let mut ends = Vec::with_capacity(count);
    for i in 0..count {
        let start = (i as u32) * 10;
        starts.push(start);
        ends.push(start + 1);
    }
    starts[300] = 3_000;
    ends[300] = 3_010;
    starts[301] = 3_002;
    ends[301] = 3_003;
    starts[302] = 3_009;
    ends[302] = 3_015;
    starts[303] = 3_020;
    ends[303] = 3_021;

    let (cpu_flags, cpu_merged) = cpu_dedup_cluster_metadata(&pids, &starts, &ends);
    let (gpu_flags, gpu_merged) = run_dedup_cluster(&pids, &starts, &ends);

    assert_eq!(region_dedup_dispatch_grid(count as u32), [3, 1, 1]);
    assert_eq!(gpu_flags, cpu_flags);
    assert_eq!(gpu_merged, cpu_merged);
    assert_eq!(&gpu_flags[300..304], &[1, 0, 0, 1]);
    assert_eq!(gpu_merged[300], 3_015);
}

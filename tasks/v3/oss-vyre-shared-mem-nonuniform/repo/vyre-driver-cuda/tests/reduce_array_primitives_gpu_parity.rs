//! Parity tests for vyre-primitives reduce::{gather, scatter, histogram,
//! radix_sort, segment_reduce_sum}.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::reduce::gather::{cpu_ref as gather_cpu, gather};
use vyre_primitives::reduce::histogram::{cpu_ref as hist_cpu, histogram};
use vyre_primitives::reduce::radix_sort::{cpu_ref as radix_cpu, radix_sort};
use vyre_primitives::reduce::scatter::{cpu_ref as scatter_cpu, scatter};
use vyre_primitives::reduce::segment_reduce::{cpu_ref as seg_cpu, segment_reduce_sum};

// ---------------------------------------------------------------------
// gather
// ---------------------------------------------------------------------

fn run_gather(src: &[u32], indices: &[u32]) -> Vec<u32> {
    let count = src.len() as u32;
    let program = gather("src", "indices", "dst", count);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(src),
        u32_bytes(indices),
        vec![0u8; count as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("gather primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA gather dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(count as usize);
    out
}

#[test]
fn cuda_gather_identity() {
    let src = vec![10u32, 20, 30, 40];
    let indices = vec![0u32, 1, 2, 3];
    let cpu = gather_cpu(&src, &indices);
    let gpu = run_gather(&src, &indices);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, src);
}

#[test]
fn cuda_gather_reverse() {
    let src = vec![10u32, 20, 30, 40];
    let indices = vec![3u32, 2, 1, 0];
    let cpu = gather_cpu(&src, &indices);
    let gpu = run_gather(&src, &indices);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![40, 30, 20, 10]);
}

// ---------------------------------------------------------------------
// scatter
// ---------------------------------------------------------------------

fn run_scatter(src: &[u32], indices: &[u32]) -> Vec<u32> {
    let count = src.len() as u32;
    let program = scatter("src", "indices", "dst", count);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(src),
        u32_bytes(indices),
        vec![0u8; count as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("scatter primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA scatter dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(count as usize);
    out
}

#[test]
fn cuda_scatter_inverse_of_gather() {
    let src = vec![10u32, 20, 30, 40];
    let indices = vec![3u32, 2, 1, 0];
    let cpu = scatter_cpu(&src, &indices, src.len());
    let gpu = run_scatter(&src, &indices);
    assert_eq!(gpu, cpu);
    // Each src[i] is written to dst[indices[i]] → reversed source.
    assert_eq!(gpu, vec![40, 30, 20, 10]);
}

#[test]
fn cuda_scatter_identity() {
    let src = vec![5u32, 6, 7, 8];
    let indices = vec![0u32, 1, 2, 3];
    let cpu = scatter_cpu(&src, &indices, src.len());
    let gpu = run_scatter(&src, &indices);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, src);
}

// ---------------------------------------------------------------------
// histogram
// ---------------------------------------------------------------------

fn run_histogram(input: &[u32], num_bins: u32) -> Vec<u32> {
    let count = input.len() as u32;
    let program = histogram("input", "output", count, num_bins);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(input), vec![0u8; num_bins as usize * 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("histogram primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA histogram dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(num_bins as usize);
    out
}

#[test]
fn cuda_histogram_simple() {
    let input = vec![0u32, 1, 0, 2, 1, 0];
    let num_bins = 4u32;
    let cpu = hist_cpu(&input, num_bins);
    let gpu = run_histogram(&input, num_bins);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![3, 2, 1, 0]);
}

#[test]
fn cuda_histogram_skips_out_of_range() {
    // bin index 5 exceeds num_bins=4 → cpu_ref skips it.
    let input = vec![0u32, 5, 1, 5, 2];
    let num_bins = 4u32;
    let cpu = hist_cpu(&input, num_bins);
    let gpu = run_histogram(&input, num_bins);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 1, 1, 0]);
}

// ---------------------------------------------------------------------
// radix_sort
// ---------------------------------------------------------------------

fn run_radix_sort(input: &[u32], bits: u32) -> Vec<u32> {
    let count = input.len() as u32;
    let program = radix_sort("input", "output", count, bits);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(input), vec![0u8; count as usize * 4]];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("radix sort primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA radix-sort dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(count as usize);
    out
}

#[test]
fn cuda_radix_sort_already_sorted() {
    let v = vec![1u32, 2, 3, 4, 5];
    let cpu = radix_cpu(&v, 8);
    let gpu = run_radix_sort(&v, 8);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, v);
}

#[test]
fn cuda_radix_sort_reverse() {
    let v = vec![5u32, 4, 3, 2, 1];
    let cpu = radix_cpu(&v, 8);
    let gpu = run_radix_sort(&v, 8);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 2, 3, 4, 5]);
}

#[test]
fn cuda_radix_sort_with_duplicates() {
    let v = vec![3u32, 1, 4, 1, 5, 9, 2, 6, 5, 3];
    let cpu = radix_cpu(&v, 8);
    let gpu = run_radix_sort(&v, 8);
    assert_eq!(gpu, cpu);
    let mut expected = v.clone();
    expected.sort_unstable();
    assert_eq!(gpu, expected);
}

// ---------------------------------------------------------------------
// segment_reduce_sum
// ---------------------------------------------------------------------

fn run_segment_reduce(input: &[u32], segment_offsets: &[u32]) -> Vec<u32> {
    let num_segments = (segment_offsets.len() - 1) as u32;
    let program = segment_reduce_sum("input", "segments", "output", num_segments);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(input),
        u32_bytes(segment_offsets),
        vec![0u8; num_segments as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((num_segments + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("segment reduce primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA segment-reduce dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(num_segments as usize);
    out
}

#[test]
fn cuda_segment_reduce_uniform_segments() {
    let input = vec![1u32, 2, 3, 4, 5, 6];
    // Segments [0..2), [2..4), [4..6).
    let segments = vec![0u32, 2, 4, 6];
    let cpu = seg_cpu(&input, &segments);
    let gpu = run_segment_reduce(&input, &segments);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![3, 7, 11]);
}

#[test]
fn cuda_segment_reduce_uneven_segments() {
    let input = vec![10u32, 20, 30, 40, 50];
    let segments = vec![0u32, 1, 4, 5];
    let cpu = seg_cpu(&input, &segments);
    let gpu = run_segment_reduce(&input, &segments);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![10, 90, 50]);
}

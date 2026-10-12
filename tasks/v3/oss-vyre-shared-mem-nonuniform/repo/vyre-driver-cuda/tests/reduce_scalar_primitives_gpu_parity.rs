//! Parity tests for vyre-primitives reduce::{all, any, count, count_non_zero,
//! max, min, sum, range_counts_u32, workgroup_any_u32}.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::reduce::all::{cpu_ref as all_cpu, reduce_all};
use vyre_primitives::reduce::any::{cpu_ref as any_cpu, reduce_any};
use vyre_primitives::reduce::count::{cpu_ref as count_cpu, reduce_count};
use vyre_primitives::reduce::count_non_zero::{cpu_ref as count_nz_cpu, reduce_count_non_zero};
use vyre_primitives::reduce::max::{cpu_ref as max_cpu, reduce_max};
use vyre_primitives::reduce::min::{cpu_ref as min_cpu, reduce_min};
use vyre_primitives::reduce::range_counts::{cpu_ref as range_counts_cpu, range_counts_u32};
use vyre_primitives::reduce::sum::{cpu_ref as sum_cpu, reduce_sum};
use vyre_primitives::reduce::workgroup_any::workgroup_any_u32;

fn run_scalar_reduce<B>(builder: B, values: &[u32]) -> u32
where
    B: FnOnce(&str, &str, u32) -> vyre::Program,
{
    let count = values.len() as u32;
    let program = builder("values", "out", count);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(values), vec![0u8; 4]];
    let mut config = DispatchConfig::default();
    // workgroup [1,1,1].
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("scalar reduce primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA scalar reduce dispatch failed: {error}"))
    });
    bytes_u32(&outputs[0])[0]
}

#[test]
fn cuda_reduce_all_with_zero_returns_zero() {
    let v = vec![1u32, 1, 0, 1];
    let cpu = all_cpu(&v);
    let gpu = run_scalar_reduce(reduce_all, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 0);
}

#[test]
fn cuda_reduce_all_all_set_returns_one() {
    let v = vec![1u32; 8];
    let cpu = all_cpu(&v);
    let gpu = run_scalar_reduce(reduce_all, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 1);
}

#[test]
fn cuda_reduce_any_with_one_returns_one() {
    let v = vec![0u32, 0, 1, 0];
    let cpu = any_cpu(&v);
    let gpu = run_scalar_reduce(reduce_any, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 1);
}

#[test]
fn cuda_reduce_any_all_zero_returns_zero() {
    let v = vec![0u32; 8];
    let cpu = any_cpu(&v);
    let gpu = run_scalar_reduce(reduce_any, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 0);
}

#[test]
fn cuda_reduce_max() {
    let v = vec![3u32, 7, 1, 9, 2, 5];
    let cpu = max_cpu(&v);
    let gpu = run_scalar_reduce(reduce_max, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 9);
}

#[test]
fn cuda_reduce_min() {
    let v = vec![3u32, 7, 1, 9, 2, 5];
    let cpu = min_cpu(&v);
    let gpu = run_scalar_reduce(reduce_min, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 1);
}

#[test]
fn cuda_reduce_sum() {
    let v = vec![1u32, 2, 3, 4, 5];
    let cpu = sum_cpu(&v);
    let gpu = run_scalar_reduce(reduce_sum, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 15);
}

#[test]
fn cuda_reduce_sum_with_overflow_wraps() {
    let v = vec![u32::MAX, 1u32];
    let cpu = sum_cpu(&v);
    let gpu = run_scalar_reduce(reduce_sum, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 0);
}

#[test]
fn cuda_reduce_count_non_zero() {
    let v = vec![0u32, 5, 0, 7, 0, 0, 3];
    let cpu = count_nz_cpu(&v);
    let gpu = run_scalar_reduce(reduce_count_non_zero, &v);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 3);
}

#[test]
fn cuda_reduce_count_bitset_popcount() {
    // reduce_count counts set bits in the packed bitset.
    let bits = vec![0b1010u32, 0xFFu32, 0u32];
    let cpu = count_cpu(&bits);
    let gpu = run_scalar_reduce(reduce_count, &bits);
    assert_eq!(gpu, cpu);
    // 0b1010 has 2 bits, 0xFF has 8, 0 has 0 → total 10.
    assert_eq!(gpu, 10);
}

// ---------------------------------------------------------------------
// range_counts_u32 (output buffer, no input slot)
// ---------------------------------------------------------------------

fn run_range_counts(histogram: &[u32; 256], start: u32, end: u32) -> u32 {
    let program = range_counts_u32("histogram", "out", start, end);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(histogram)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("range counts primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA range-counts dispatch failed: {error}"))
    });
    bytes_u32(&outputs[0])[0]
}

#[test]
fn cuda_range_counts_ascii_band() {
    let mut histogram = [0u32; 256];
    histogram[b'A' as usize] = 3;
    histogram[b'Z' as usize] = 5;
    histogram[0xFF] = 99;
    let cpu = range_counts_cpu(&histogram, 0x41, 0x5B);
    let gpu = run_range_counts(&histogram, 0x41, 0x5B);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 8); // A + Z, 0xFF excluded.
}

#[test]
fn cuda_range_counts_empty_range() {
    let mut histogram = [0u32; 256];
    histogram[0x10] = 5;
    let cpu = range_counts_cpu(&histogram, 0x20, 0x20);
    let gpu = run_range_counts(&histogram, 0x20, 0x20);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, 0);
}

// ---------------------------------------------------------------------
// workgroup_any_u32 (output buffer, no input slot)
// ---------------------------------------------------------------------

fn run_workgroup_any(values: &[u32]) -> u32 {
    let count = values.len() as u32;
    let program = workgroup_any_u32("values", "out", count);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(values)];
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("workgroup any primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA workgroup-any dispatch failed: {error}"))
    });
    bytes_u32(&outputs[0])[0]
}

#[test]
fn cuda_workgroup_any_zero_when_empty() {
    let v = vec![0u32; 16];
    let gpu = run_workgroup_any(&v);
    assert_eq!(gpu, 0);
}

#[test]
fn cuda_workgroup_any_one_when_present() {
    let mut v = vec![0u32; 16];
    v[10] = 1;
    let gpu = run_workgroup_any(&v);
    assert_eq!(gpu, 1);
}

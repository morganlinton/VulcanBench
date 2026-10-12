//! Parity test: vyre-primitives math primitives match CPU oracles.
//! Covers prefix_scan (inclusive + exclusive sum) and stream_compact.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::math::prefix_scan::{
    cpu_ref as prefix_cpu, prefix_scan, prefix_scan_large, ScanKind,
};
use vyre_primitives::math::stream_compact::{cpu_ref as compact_cpu, stream_compact};
use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

fn run_prefix_scan(input: &[u32], kind: ScanKind) -> Vec<u32> {
    let n = input.len() as u32;
    let program = prefix_scan("in", "out", n, kind);
    let inputs: Vec<Vec<u8>> = vec![u32_bytes(input)];
    let mut config = DispatchConfig::default();
    // Workgroup is `n.next_power_of_two()`, 1 workgroup.
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("prefix scan primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA prefix-scan dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(n as usize);
    out
}

fn run_prefix_scan_large(input: &[u32]) -> Vec<u32> {
    use vyre::ir::BufferAccess;

    let n = input.len() as u32;
    let program = prefix_scan_large("in", "out", n);
    let mut inputs: Vec<Vec<u8>> = Vec::new();
    for buffer in program.buffers() {
        let backend_allocated = buffer.is_output() || buffer.is_pipeline_live_out();
        let needs_input = matches!(
            buffer.access(),
            BufferAccess::ReadOnly | BufferAccess::ReadWrite
        ) && !backend_allocated
            && !matches!(buffer.access(), BufferAccess::Workgroup);
        if needs_input {
            if buffer.name() == "in" {
                inputs.push(u32_bytes(input));
            } else {
                inputs.push(vec![0u8; buffer.count() as usize * 4]);
            }
        }
    }

    let outputs = with_live_backend("large prefix scan primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &DispatchConfig::default())
            .unwrap_or_else(|error| panic!("Fix: CUDA large prefix-scan dispatch failed: {error}"))
    });
    let out_idx = program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|buffer| buffer.name() == "out")
        .expect("Fix: large prefix scan output buffer must be declared");
    let mut out = bytes_u32(&outputs[out_idx]);
    out.truncate(n as usize);
    out
}

#[test]
fn cuda_prefix_scan_inclusive_sum() {
    let input = vec![1u32, 2, 3, 4];
    let cpu = prefix_cpu(&input, ScanKind::InclusiveSum);
    let gpu = run_prefix_scan(&input, ScanKind::InclusiveSum);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 3, 6, 10]);
}

#[test]
fn cuda_prefix_scan_exclusive_sum() {
    let input = vec![1u32, 2, 3, 4];
    let cpu = prefix_cpu(&input, ScanKind::ExclusiveSum);
    let gpu = run_prefix_scan(&input, ScanKind::ExclusiveSum);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0, 1, 3, 6]);
}

#[test]
fn cuda_prefix_scan_inclusive_zeros() {
    let input = vec![0u32; 8];
    let cpu = prefix_cpu(&input, ScanKind::InclusiveSum);
    let gpu = run_prefix_scan(&input, ScanKind::InclusiveSum);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32; 8]);
}

#[test]
fn cuda_prefix_scan_inclusive_non_pow2() {
    let input = vec![5u32, 1, 4, 1, 5, 9, 2];
    let cpu = prefix_cpu(&input, ScanKind::InclusiveSum);
    let gpu = run_prefix_scan(&input, ScanKind::InclusiveSum);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_prefix_scan_large_crosses_block_boundary() {
    let len = BLOCK_LANES as usize + 33;
    let input: Vec<u32> = (0..len)
        .map(|idx| {
            (idx as u32)
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
        })
        .collect();
    let cpu = prefix_cpu(&input, ScanKind::InclusiveSum);
    let gpu = run_prefix_scan_large(&input);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu[BLOCK_LANES as usize - 1], cpu[BLOCK_LANES as usize - 1]);
    assert_eq!(gpu[BLOCK_LANES as usize], cpu[BLOCK_LANES as usize]);
    assert_eq!(gpu[len - 1], cpu[len - 1]);
}

fn run_stream_compact(payloads: &[u32], flags: &[u32]) -> (Vec<u32>, u32) {
    let count = payloads.len() as u32;
    // offsets must be exclusive-sum-of-flags (host-precomputed); the
    // GPU stream_compact primitive expects the offsets as input.
    let mut offsets = vec![0u32; count as usize];
    let mut acc = 0u32;
    for i in 0..count as usize {
        offsets[i] = acc;
        acc = acc.saturating_add(flags[i]);
    }
    let program = stream_compact(
        "payloads",
        "flags",
        "offsets",
        "compacted",
        "live_count",
        count,
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(payloads),
        u32_bytes(flags),
        u32_bytes(&offsets),
        // compacted: zero-init, length count.
        vec![0u8; count as usize * 4],
        // live_count: zero-init.
        vec![0u8; 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("stream compact primitive", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA stream-compact dispatch failed: {error}"))
    });
    let compacted = bytes_u32(&outputs[0]);
    let live_count = bytes_u32(&outputs[1])[0];
    let live_compacted = compacted[..live_count as usize].to_vec();
    (live_compacted, live_count)
}

#[test]
fn cuda_stream_compact_keeps_live_lanes_in_order() {
    let payloads = vec![10u32, 20, 30, 40, 50];
    let flags = vec![0u32, 1, 1, 0, 1];
    let cpu = compact_cpu(&payloads, &flags);
    let gpu = run_stream_compact(&payloads, &flags);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.0, vec![20, 30, 50]);
    assert_eq!(gpu.1, 3);
}

#[test]
fn cuda_stream_compact_all_dead() {
    let payloads = vec![1u32, 2, 3];
    let flags = vec![0u32, 0, 0];
    let cpu = compact_cpu(&payloads, &flags);
    let gpu = run_stream_compact(&payloads, &flags);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.1, 0);
    assert!(gpu.0.is_empty());
}

#[test]
fn cuda_stream_compact_all_live() {
    let payloads = vec![100u32, 200, 300, 400];
    let flags = vec![1u32, 1, 1, 1];
    let cpu = compact_cpu(&payloads, &flags);
    let gpu = run_stream_compact(&payloads, &flags);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu.1, 4);
    assert_eq!(gpu.0, payloads);
}

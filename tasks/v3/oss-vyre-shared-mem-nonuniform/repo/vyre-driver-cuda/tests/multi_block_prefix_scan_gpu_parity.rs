//! Parity test for reduce::multi_block_prefix_scan_sum_u32  -  covers
//! both the small-input fast path (n ≤ BLOCK_LANES) and the multi-pass
//! Blelloch chain.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::reduce::multi_block_prefix_scan::{
    cpu_ref as mbps_cpu, multi_block_prefix_scan_sum_u32, pass_c_broadcast_offsets, BLOCK_LANES,
};

fn run_mbps(input: &[u32]) -> Vec<u32> {
    use vyre::ir::BufferAccess;
    let n = input.len() as u32;
    let program = multi_block_prefix_scan_sum_u32("input", "output", n);
    // Only ReadOnly / ReadWrite storage buffers consume input slots.
    // BufferDecl::output (WriteOnly) and BufferDecl::workgroup do not.
    let mut inputs: Vec<Vec<u8>> = Vec::new();
    for buf in program.buffers().iter() {
        let access = buf.access();
        // Output buffers, pipeline-live intermediates, and workgroup-local
        // scratch are backend-allocated and do not take input slots.
        let backend_allocated = buf.is_output() || buf.is_pipeline_live_out();
        let needs_input = matches!(access, BufferAccess::ReadOnly | BufferAccess::ReadWrite)
            && !backend_allocated
            && !matches!(access, BufferAccess::Workgroup);
        if !needs_input {
            continue;
        }
        let elements = buf.count() as usize;
        if buf.name() == "input" {
            inputs.push(u32_bytes(input));
        } else {
            inputs.push(vec![0u8; elements.max(1) * 4]);
        }
    }
    let mut config = DispatchConfig::default();
    // Small fast path uses `prefix_scan` workgroup; large path uses
    // multi-block dispatch where the substrate handles grid_x.
    if n <= BLOCK_LANES {
        config.grid_override = Some([1, 1, 1]);
    }
    let outputs = with_live_backend("multi-block prefix scan", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA multi-block prefix-scan dispatch failed: {error}")
            })
    });
    // Outputs are returned in declaration order over writeable buffers
    // (RW storage + WriteOnly + is_output marker). Locate "output".
    let out_idx = program
        .buffers()
        .iter()
        .filter(|b| {
            b.is_output()
                || matches!(
                    b.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|b| b.name() == "output")
        .expect("output buffer present");
    let mut out = bytes_u32(&outputs[out_idx]);
    out.truncate(n as usize);
    out
}

#[test]
fn cuda_mbps_small_inclusive_sum() {
    let input = vec![1u32, 2, 3, 4, 5];
    let cpu = mbps_cpu(&input);
    let gpu = run_mbps(&input);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![1, 3, 6, 10, 15]);
}

#[test]
fn cuda_mbps_zeros() {
    let input = vec![0u32; 16];
    let cpu = mbps_cpu(&input);
    let gpu = run_mbps(&input);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32; 16]);
}

#[test]
fn cuda_mbps_one_full_block() {
    // Exactly BLOCK_LANES elements, all ones; inclusive scan = 1..=BLOCK_LANES.
    let input = vec![1u32; BLOCK_LANES as usize];
    let cpu = mbps_cpu(&input);
    let gpu = run_mbps(&input);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu[BLOCK_LANES as usize - 1], BLOCK_LANES);
}

#[test]
fn cuda_mbps_crosses_block_boundary() {
    let len = BLOCK_LANES as usize + 17;
    let input: Vec<u32> = (0..len)
        .map(|index| {
            let index = index as u32;
            index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
        })
        .collect();
    let cpu = mbps_cpu(&input);
    let gpu = run_mbps(&input);

    assert_eq!(gpu, cpu);
    assert_eq!(gpu[BLOCK_LANES as usize - 1], cpu[BLOCK_LANES as usize - 1]);
    assert_eq!(gpu[BLOCK_LANES as usize], cpu[BLOCK_LANES as usize]);
    assert_eq!(gpu[len - 1], cpu[len - 1]);
}

#[test]
fn cuda_mbps_pass_c_adds_scanned_block_offset() {
    let len = BLOCK_LANES as usize + 17;
    let num_blocks = 2;
    let mut partials = vec![0u32; BLOCK_LANES as usize * num_blocks as usize];
    for (index, slot) in partials.iter_mut().enumerate().take(len) {
        *slot = (index as u32 % BLOCK_LANES).wrapping_add(1);
    }
    let scanned_totals = vec![7_000u32, 99_000u32];
    let program = pass_c_broadcast_offsets(
        "partials",
        "block_totals_scanned",
        "output",
        len as u32,
        num_blocks,
    );
    let inputs = vec![u32_bytes(&partials), u32_bytes(&scanned_totals)];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let outputs = with_live_backend("multi-block prefix scan pass C", |backend| {
        backend
            .dispatch_borrowed(&program, &input_refs, &DispatchConfig::default())
            .unwrap_or_else(|error| panic!("Fix: CUDA Pass-C dispatch failed: {error}"))
    });
    let gpu = bytes_u32(&outputs[0]);

    assert_eq!(gpu[0], 1);
    assert_eq!(gpu[BLOCK_LANES as usize - 1], BLOCK_LANES);
    assert_eq!(gpu[BLOCK_LANES as usize], 7_001);
    assert_eq!(gpu[len - 1], 7_017);
}

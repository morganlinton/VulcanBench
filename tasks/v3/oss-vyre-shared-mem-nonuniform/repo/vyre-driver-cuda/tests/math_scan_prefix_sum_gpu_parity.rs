//! CUDA parity for the public vyre-libs prefix-sum composition.
//!
//! This covers the large-input route that must dispatch through the
//! multi-block scan chain rather than the historical single-lane loop.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::ir::{BufferAccess, Program};
use vyre::DispatchConfig;
use vyre_libs::math::scan_prefix_sum;

fn scan_inputs_for_program(program: &Program, input: &[u32]) -> Vec<Vec<u8>> {
    program
        .buffers()
        .iter()
        .filter_map(|buffer| {
            let backend_allocated = buffer.is_output() || buffer.is_pipeline_live_out();
            let needs_input = matches!(
                buffer.access(),
                BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
            ) && !backend_allocated
                && buffer.access() != BufferAccess::Workgroup;
            if !needs_input {
                return None;
            }
            if buffer.name() == "input" {
                Some(u32_bytes(input))
            } else {
                Some(vec![0u8; buffer.count().max(1) as usize * 4])
            }
        })
        .collect()
}

fn produced_buffer_index(program: &Program, name: &str) -> usize {
    program
        .buffers()
        .iter()
        .filter(|buffer| {
            buffer.is_output()
                || buffer.is_pipeline_live_out()
                || matches!(
                    buffer.access(),
                    BufferAccess::ReadWrite | BufferAccess::WriteOnly
                )
        })
        .position(|buffer| buffer.name() == name)
        .unwrap_or_else(|| panic!("Fix: CUDA scan output buffer `{name}` must be declared"))
}

fn cpu_wrapping_prefix_sum(input: &[u32]) -> Vec<u32> {
    let mut acc = 0u32;
    input
        .iter()
        .map(|&value| {
            acc = acc.wrapping_add(value);
            acc
        })
        .collect()
}

fn run_scan_prefix_sum(input: &[u32]) -> Vec<u32> {
    let n = input.len() as u32;
    let program = scan_prefix_sum("input", "output", n);
    let inputs = scan_inputs_for_program(&program, input);
    let output_index = produced_buffer_index(&program, "output");
    let outputs = with_live_backend("vyre-libs scan_prefix_sum", |backend| {
        backend
            .dispatch(&program, &inputs, &DispatchConfig::default())
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA vyre-libs scan_prefix_sum dispatch failed: {error}")
            })
    });
    let mut output = bytes_u32(&outputs[output_index]);
    output.truncate(input.len());
    output
}

fn patterned_input(len: usize) -> Vec<u32> {
    (0..len as u32)
        .map(|index| {
            index
                .wrapping_mul(2_654_435_761)
                .wrapping_add(1_013_904_223)
        })
        .collect()
}

fn assert_scan_prefix_sum_matches_cpu(len: usize) {
    let input = patterned_input(len);
    let cpu = cpu_wrapping_prefix_sum(&input);
    let gpu = run_scan_prefix_sum(&input);

    assert_eq!(gpu.len(), len, "len={len}");
    for (index, (&actual, &expected)) in gpu.iter().zip(cpu.iter()).enumerate() {
        assert_eq!(actual, expected, "len={len} index={index}");
    }
}

#[test]
fn cuda_vyre_libs_scan_prefix_sum_large_boundary_matrix() {
    for len in [1025, 2048, 2049, 4099, 8193] {
        assert_scan_prefix_sum_matches_cpu(len);
    }
}

#[test]
fn cuda_vyre_libs_scan_prefix_sum_recursive_block_totals() {
    assert_scan_prefix_sum_matches_cpu(1024 * 1024 + 17);
}

//! Parity test: vyre-primitives bitset primitives match CPU oracles.
//! Covers bitset_not, _or, _equal, _subset_of, _test_bit, _contains,
//! _copy, _zero, _and_into, _or_into, _xor_into, _and_not_into, _clear_bit.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::bitset::and_into::{bitset_and_into, cpu_ref as and_into_cpu};
use vyre_primitives::bitset::and_not_into::{bitset_and_not_into, cpu_ref as and_not_into_cpu};
use vyre_primitives::bitset::clear_bit::{bitset_clear_bit, cpu_ref as clear_bit_cpu};
use vyre_primitives::bitset::contains::{bitset_contains, cpu_ref as contains_cpu};
use vyre_primitives::bitset::copy::{bitset_copy, cpu_ref as copy_cpu};
use vyre_primitives::bitset::equal::{bitset_equal, cpu_ref as equal_cpu};
use vyre_primitives::bitset::frontier::{
    absorb_new_frontier_bits, frontier_absorb_new_bits_no_counts_for_node_count_program,
};
use vyre_primitives::bitset::not::{bitset_not, cpu_ref as not_cpu};
use vyre_primitives::bitset::or::{bitset_or, cpu_ref as or_cpu};
use vyre_primitives::bitset::or_into::{bitset_or_into, cpu_ref as or_into_cpu};
use vyre_primitives::bitset::subset_of::{bitset_subset_of, cpu_ref as subset_of_cpu};
use vyre_primitives::bitset::test_bit::{bitset_test_bit, cpu_ref as test_bit_cpu};
use vyre_primitives::bitset::xor_into::{bitset_xor_into, cpu_ref as xor_into_cpu};
use vyre_primitives::bitset::zero::{bitset_zero, cpu_ref as zero_cpu};

fn dispatch_grid(program: &vyre::ir::Program, inputs: &[Vec<u8>], grid_x: u32) -> Vec<Vec<u8>> {
    let mut config = DispatchConfig::default();
    config.grid_override = Some([grid_x.max(1), 1, 1]);
    with_live_backend("bitset primitive dispatch", |backend| {
        backend
            .dispatch(program, inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA bitset primitive dispatch failed: {error}"))
    })
}

#[test]
fn cuda_bitset_not_parity() {
    let input = vec![0x0F0F_0F0Fu32, 0xCAFE_BABE];
    let cpu = not_cpu(&input);
    let program = bitset_not("input", "out", 2);
    let inputs = vec![u32_bytes(&input), vec![0u8; 8]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_bitset_or_parity() {
    let lhs = vec![0xFF00u32, 0x0F0F];
    let rhs = vec![0x00FFu32, 0xF0F0];
    let cpu = or_cpu(&lhs, &rhs);
    let program = bitset_or("lhs", "rhs", "out", 2);
    let inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 8]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_bitset_equal_parity() {
    let lhs = vec![0xDEADu32, 0xBEEF];
    let rhs = vec![0xDEADu32, 0xBEEF];
    let expected = equal_cpu(&lhs, &rhs);
    let program = bitset_equal("lhs", "rhs", "out", 2);
    let inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 4]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let scalar = bytes_u32(&outputs[0])[0];
    assert_eq!(scalar, expected);
    assert_eq!(scalar, 1);

    let rhs_diff = vec![0xDEAEu32, 0xBEEF];
    let inputs2 = vec![u32_bytes(&lhs), u32_bytes(&rhs_diff), vec![0u8; 4]];
    let outputs2 = dispatch_grid(&program, &inputs2, 1);
    let scalar2 = bytes_u32(&outputs2[0])[0];
    assert_eq!(scalar2, equal_cpu(&lhs, &rhs_diff));
    assert_eq!(scalar2, 0);
}

#[test]
fn cuda_bitset_equal_crosses_workgroup_lanes() {
    let lhs = vec![0xFFFF_FFFFu32; 600];
    let mut rhs = lhs.clone();
    rhs[513] ^= 1;
    let program = bitset_equal("lhs", "rhs", "out", lhs.len() as u32);
    let inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 4]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    assert_eq!(bytes_u32(&outputs[0])[0], equal_cpu(&lhs, &rhs));
}

#[test]
fn cuda_bitset_subset_of_parity() {
    let lhs = vec![0b0011u32];
    let rhs = vec![0b1111u32];
    let cpu = subset_of_cpu(&lhs, &rhs);
    let program = bitset_subset_of("lhs", "rhs", "out", 1);
    let inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 4]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    assert_eq!(bytes_u32(&outputs[0])[0], cpu);
    assert_eq!(cpu, 1);
}

#[test]
fn cuda_bitset_subset_of_crosses_workgroup_lanes() {
    let lhs = vec![0u32; 600];
    let mut rhs = vec![0u32; 600];
    rhs[511] = 0xFFFF_FFFF;
    let mut not_subset = lhs.clone();
    not_subset[513] = 0b1000;
    let program = bitset_subset_of("lhs", "rhs", "out", lhs.len() as u32);
    let ok_inputs = vec![u32_bytes(&lhs), u32_bytes(&rhs), vec![0u8; 4]];
    let ok_outputs = dispatch_grid(&program, &ok_inputs, 1);
    assert_eq!(bytes_u32(&ok_outputs[0])[0], subset_of_cpu(&lhs, &rhs));
    let bad_inputs = vec![u32_bytes(&not_subset), u32_bytes(&rhs), vec![0u8; 4]];
    let bad_outputs = dispatch_grid(&program, &bad_inputs, 1);
    assert_eq!(
        bytes_u32(&bad_outputs[0])[0],
        subset_of_cpu(&not_subset, &rhs)
    );
}

#[test]
fn cuda_bitset_test_bit_parity() {
    let buf = vec![0b1010u32];
    let bit_idx = 1u32;
    let expected = test_bit_cpu(&buf, bit_idx);
    let program = bitset_test_bit("buf", bit_idx, "out");
    let inputs = vec![u32_bytes(&buf), vec![0u8; 4]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    assert_eq!(bytes_u32(&outputs[0])[0], expected);
    assert_eq!(expected, 1);
}

#[test]
fn cuda_bitset_contains_parity() {
    let buf = vec![0b1010u32];
    let cpu = contains_cpu(&buf, 1);
    let program = bitset_contains("buf", "idx", "out", 1);
    let inputs = vec![u32_bytes(&buf), u32_bytes(&[1u32]), vec![0u8; 4]];
    let outputs = dispatch_grid(&program, &inputs, 1);
    assert_eq!(bytes_u32(&outputs[0])[0], cpu);
}

#[test]
fn cuda_bitset_copy_parity() {
    let src = vec![0xCAFEu32, 0xBABE];
    let mut cpu_dst = vec![0u32; 2];
    copy_cpu(&mut cpu_dst, &src);
    let program = bitset_copy("dst", "src", 2);
    // RW target then RO source order in declaration.
    let inputs = vec![vec![0u8; 8], u32_bytes(&src)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    assert_eq!(gpu, cpu_dst);
}

#[test]
fn cuda_bitset_zero_parity_crosses_workgroup_lanes() {
    let mut cpu_target = (0..600).map(|idx| 0xA5A5_0000u32 ^ idx).collect::<Vec<_>>();
    let program = bitset_zero("target", cpu_target.len() as u32);
    let inputs = vec![u32_bytes(&cpu_target)];
    let outputs = dispatch_grid(&program, &inputs, 3);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(cpu_target.len());
    zero_cpu(&mut cpu_target);
    assert_eq!(gpu, cpu_target);
}

#[test]
fn cuda_frontier_absorb_no_counts_masks_tail_and_emits_new_wave() {
    let node_count = 65;
    let mut visited = vec![0b0001u32, 0, 0];
    let neighbors = vec![0b0111u32, u32::MAX, u32::MAX];
    let mut expected_next = Vec::new();
    absorb_new_frontier_bits(node_count, &mut visited, &neighbors, &mut expected_next)
        .expect("Fix: frontier absorb CPU oracle should accept canonical shapes");

    let program = frontier_absorb_new_bits_no_counts_for_node_count_program(
        "visited",
        "neighbors",
        "next",
        node_count,
    );
    let inputs = vec![
        u32_bytes(&[0b0001u32, 0, 0]),
        u32_bytes(&neighbors),
        vec![0xFFu8; 12],
    ];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu_visited = bytes_u32(&outputs[0]);
    let mut gpu_next = bytes_u32(&outputs[1]);
    gpu_visited.truncate(3);
    gpu_next.truncate(3);

    assert_eq!(gpu_visited, visited);
    assert_eq!(gpu_next, expected_next);
    assert_eq!(
        gpu_next[2], 1,
        "only node 64 may survive the final-word mask"
    );
}

#[test]
fn cuda_bitset_and_into_parity() {
    let mut cpu_target = vec![0xFF00u32, 0xFFFF];
    let mask = vec![0x0F00u32, 0x0F0F];
    let program = bitset_and_into("target", "mask", 2);
    let inputs = vec![u32_bytes(&cpu_target), u32_bytes(&mask)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    and_into_cpu(&mut cpu_target, &mask);
    assert_eq!(gpu, cpu_target);
}

#[test]
fn cuda_bitset_or_into_parity() {
    let mut cpu_target = vec![0xFF00u32, 0x0F0F];
    let addend = vec![0x00FFu32, 0xF0F0];
    let program = bitset_or_into("target", "addend", 2);
    let inputs = vec![u32_bytes(&cpu_target), u32_bytes(&addend)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    or_into_cpu(&mut cpu_target, &addend);
    assert_eq!(gpu, cpu_target);
}

#[test]
fn cuda_bitset_xor_into_parity() {
    let mut cpu_target = vec![0xCAFEu32, 0xBABE];
    let addend = vec![0xDEADu32, 0xBEEF];
    let program = bitset_xor_into("target", "addend", 2);
    let inputs = vec![u32_bytes(&cpu_target), u32_bytes(&addend)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    xor_into_cpu(&mut cpu_target, &addend);
    assert_eq!(gpu, cpu_target);
}

#[test]
fn cuda_bitset_and_not_into_parity() {
    let mut cpu_target = vec![0xFFFFu32, 0x00FF];
    let subtrahend = vec![0xFF00u32, 0x00F0];
    let program = bitset_and_not_into("target", "subtrahend", 2);
    let inputs = vec![u32_bytes(&cpu_target), u32_bytes(&subtrahend)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    and_not_into_cpu(&mut cpu_target, &subtrahend);
    assert_eq!(gpu, cpu_target);
}

#[test]
fn cuda_bitset_clear_bit_parity() {
    let mut cpu_target = vec![0xFFFFu32, 0xFFFF];
    let bit_idx = 5u32;
    let program = bitset_clear_bit("target", bit_idx, 2);
    let inputs = vec![u32_bytes(&cpu_target)];
    let outputs = dispatch_grid(&program, &inputs, 1);
    let mut gpu = bytes_u32(&outputs[0]);
    gpu.truncate(2);
    clear_bit_cpu(&mut cpu_target, bit_idx);
    assert_eq!(gpu, cpu_target);
}

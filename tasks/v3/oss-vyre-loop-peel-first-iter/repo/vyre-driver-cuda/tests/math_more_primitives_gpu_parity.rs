//! Parity tests for vyre-primitives math primitives:
//! bigint_add_carry, interval_merge, argmax_of_marginals.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::math::bigint_add_carry::{
    bigint_add_carry, bigint_add_carry_cpu, bigint_add_carry_dispatch_grid, BINDING_A_IN,
    BINDING_B_IN, BINDING_CARRY_PARTIAL_OUT, BINDING_SUM_PARTIAL_OUT,
};
use vyre_primitives::math::interval::{cpu_interval_merge, interval_merge_program};
use vyre_primitives::math::submodular_greedy::{
    argmax_of_marginals, argmax_of_marginals_cpu, NO_WINNER,
};

fn run_bigint_add_carry(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    assert_eq!(a.len(), b.len());
    let limb_count = a.len() as u32;
    let program = bigint_add_carry(limb_count);
    let mut inputs: Vec<Vec<u8>> = vec![Vec::new(); 4];
    inputs[BINDING_A_IN as usize] = u32_bytes(a);
    inputs[BINDING_B_IN as usize] = u32_bytes(b);
    inputs[BINDING_SUM_PARTIAL_OUT as usize] = vec![0u8; limb_count as usize * 4];
    inputs[BINDING_CARRY_PARTIAL_OUT as usize] = vec![0u8; limb_count as usize * 4];
    let mut config = DispatchConfig::default();
    config.grid_override = Some(bigint_add_carry_dispatch_grid(limb_count));
    let outputs = with_live_backend("bigint add carry", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA bigint add-carry dispatch failed: {error}"))
    });
    // RW outputs are returned in declaration order; A and B are ReadOnly,
    // so outputs[0]=sum_partial, outputs[1]=carry_partial.
    let mut sum = bytes_u32(&outputs[0]);
    let mut carry = bytes_u32(&outputs[1]);
    sum.truncate(limb_count as usize);
    carry.truncate(limb_count as usize);
    (sum, carry)
}

#[test]
fn cuda_bigint_add_carry_no_overflow() {
    let a = vec![1u32, 2, 3, 4];
    let b = vec![10u32, 20, 30, 40];
    let (cpu_sum, cpu_carry) = bigint_add_carry_cpu(&a, &b).expect("ok");
    let (gpu_sum, gpu_carry) = run_bigint_add_carry(&a, &b);
    assert_eq!(gpu_sum, cpu_sum);
    assert_eq!(gpu_carry, cpu_carry);
    assert_eq!(gpu_carry, vec![0u32; 4]);
}

#[test]
fn cuda_bigint_add_carry_with_overflow() {
    // Each limb wraps: 0xFFFF_FFFF + 1 → carry.
    let a = vec![u32::MAX, u32::MAX, 0u32];
    let b = vec![1u32, 1u32, 1u32];
    let (cpu_sum, cpu_carry) = bigint_add_carry_cpu(&a, &b).expect("ok");
    let (gpu_sum, gpu_carry) = run_bigint_add_carry(&a, &b);
    assert_eq!(gpu_sum, cpu_sum);
    assert_eq!(gpu_carry, cpu_carry);
    assert_eq!(gpu_sum, vec![0, 0, 1]);
    assert_eq!(gpu_carry, vec![1, 1, 0]);
}

#[test]
fn cuda_bigint_add_carry_zero_operands() {
    let a = vec![0u32; 5];
    let b = vec![0u32; 5];
    let (cpu_sum, cpu_carry) = bigint_add_carry_cpu(&a, &b).expect("ok");
    let (gpu_sum, gpu_carry) = run_bigint_add_carry(&a, &b);
    assert_eq!(gpu_sum, cpu_sum);
    assert_eq!(gpu_carry, cpu_carry);
    assert_eq!(gpu_sum, vec![0u32; 5]);
}

#[test]
fn cuda_bigint_add_carry_multi_block_overflow_pattern() {
    let limb_count = 1025u32;
    let mut a = Vec::with_capacity(limb_count as usize);
    let mut b = Vec::with_capacity(limb_count as usize);
    for idx in 0..limb_count {
        let (left, right) = match idx {
            0 => (u32::MAX, 1),
            255 => (0, 0),
            256 => (u32::MAX, u32::MAX),
            511 => (0x8000_0000, 0x8000_0000),
            512 => (7, 9),
            1024 => (u32::MAX, 2),
            _ if idx % 5 == 0 => (u32::MAX, idx),
            _ if idx % 7 == 0 => (0x8000_0000, 0x8000_0000),
            _ => (
                idx.wrapping_mul(0x9E37_79B9),
                idx.rotate_left(11).wrapping_mul(0x85EB_CA6B),
            ),
        };
        a.push(left);
        b.push(right);
    }

    let (cpu_sum, cpu_carry) = bigint_add_carry_cpu(&a, &b).expect("ok");
    let (gpu_sum, gpu_carry) = run_bigint_add_carry(&a, &b);

    assert_eq!(bigint_add_carry_dispatch_grid(limb_count), [5, 1, 1]);
    assert_eq!(gpu_sum, cpu_sum);
    assert_eq!(gpu_carry, cpu_carry);
    assert_eq!(gpu_sum[0], 0);
    assert_eq!(gpu_carry[0], 1);
    assert_eq!(gpu_sum[256], u32::MAX - 1);
    assert_eq!(gpu_carry[256], 1);
    assert_eq!(gpu_sum[512], 16);
    assert_eq!(gpu_carry[512], 0);
    assert_eq!(gpu_sum[1024], 1);
    assert_eq!(gpu_carry[1024], 1);
}

// ---------------------------------------------------------------------
// interval_merge
// ---------------------------------------------------------------------

fn run_interval_merge(
    mins_a: &[u32],
    maxs_a: &[u32],
    mins_b: &[u32],
    maxs_b: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    let lane_count = mins_a.len() as u32;
    let program = interval_merge_program(
        "mins_a", "maxs_a", "mins_b", "maxs_b", "mins_out", "maxs_out", lane_count,
    );
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(mins_a),
        u32_bytes(maxs_a),
        u32_bytes(mins_b),
        u32_bytes(maxs_b),
        vec![0u8; lane_count as usize * 4],
        vec![0u8; lane_count as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((lane_count + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("interval merge", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA interval merge dispatch failed: {error}"))
    });
    let mut mins = bytes_u32(&outputs[0]);
    let mut maxs = bytes_u32(&outputs[1]);
    mins.truncate(lane_count as usize);
    maxs.truncate(lane_count as usize);
    (mins, maxs)
}

#[test]
fn cuda_interval_merge_basic() {
    let mins_a = vec![10u32, 0, 7];
    let maxs_a = vec![20u32, 3, 9];
    let mins_b = vec![4u32, 2, 8];
    let maxs_b = vec![18u32, 5, 12];
    let (cpu_mins, cpu_maxs) = cpu_interval_merge(&mins_a, &maxs_a, &mins_b, &maxs_b);
    let (gpu_mins, gpu_maxs) = run_interval_merge(&mins_a, &maxs_a, &mins_b, &maxs_b);
    assert_eq!(gpu_mins, cpu_mins);
    assert_eq!(gpu_maxs, cpu_maxs);
    assert_eq!(gpu_mins, vec![4, 0, 7]);
    assert_eq!(gpu_maxs, vec![20, 5, 12]);
}

#[test]
fn cuda_interval_merge_a_dominates() {
    // a fully contains b on every lane.
    let mins_a = vec![0u32, 0, 0];
    let maxs_a = vec![100u32, 100, 100];
    let mins_b = vec![10u32, 20, 30];
    let maxs_b = vec![15u32, 25, 35];
    let (cpu_mins, cpu_maxs) = cpu_interval_merge(&mins_a, &maxs_a, &mins_b, &maxs_b);
    let (gpu_mins, gpu_maxs) = run_interval_merge(&mins_a, &maxs_a, &mins_b, &maxs_b);
    assert_eq!(gpu_mins, cpu_mins);
    assert_eq!(gpu_maxs, cpu_maxs);
    assert_eq!(gpu_mins, vec![0; 3]);
    assert_eq!(gpu_maxs, vec![100; 3]);
}

// ---------------------------------------------------------------------
// argmax_of_marginals
// ---------------------------------------------------------------------

fn run_argmax(gains: &[u32], picked: &[u32]) -> (u32, u32) {
    assert_eq!(gains.len(), picked.len());
    let n = gains.len() as u32;
    let program = argmax_of_marginals("gains", "picked", "winner_idx", "winner_gain", n);
    // Initialize winner_idx to NO_WINNER and winner_gain to 0 so
    // atomic-max merging starts from a meaningful sentinel.
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(gains),
        u32_bytes(picked),
        u32_bytes(&[NO_WINNER]),
        vec![0u8; 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((n + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("argmax of marginals", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| {
                panic!("Fix: CUDA argmax-of-marginals dispatch failed: {error}")
            })
    });
    let winner_idx = bytes_u32(&outputs[0])[0];
    let winner_gain = bytes_u32(&outputs[1])[0];
    (winner_idx, winner_gain)
}

#[test]
fn cuda_argmax_of_marginals_picks_highest_unpicked() {
    let gains = vec![10u32, 50, 20, 99, 5];
    let picked = vec![0u32, 0, 0, 0, 0];
    let (cpu_idx, cpu_gain) = argmax_of_marginals_cpu(&gains, &picked);
    let (gpu_idx, gpu_gain) = run_argmax(&gains, &picked);
    assert_eq!((gpu_idx, gpu_gain), (cpu_idx, cpu_gain));
    assert_eq!(gpu_idx, 3);
    assert_eq!(gpu_gain, 99);
}

#[test]
fn cuda_argmax_of_marginals_skips_picked() {
    let gains = vec![10u32, 50, 20, 99, 5];
    // Index 3 is already picked  -  winner shifts to next-highest (50 @ 1).
    let picked = vec![0u32, 0, 0, 1, 0];
    let (cpu_idx, cpu_gain) = argmax_of_marginals_cpu(&gains, &picked);
    let (gpu_idx, gpu_gain) = run_argmax(&gains, &picked);
    assert_eq!((gpu_idx, gpu_gain), (cpu_idx, cpu_gain));
    assert_eq!(gpu_gain, 50);
}

#[test]
fn cuda_argmax_of_marginals_all_picked() {
    let gains = vec![10u32, 50, 20];
    let picked = vec![1u32, 1, 1];
    let (cpu_idx, cpu_gain) = argmax_of_marginals_cpu(&gains, &picked);
    let (gpu_idx, gpu_gain) = run_argmax(&gains, &picked);
    assert_eq!((gpu_idx, gpu_gain), (cpu_idx, cpu_gain));
    assert_eq!(gpu_idx, NO_WINNER);
    assert_eq!(gpu_gain, 0);
}

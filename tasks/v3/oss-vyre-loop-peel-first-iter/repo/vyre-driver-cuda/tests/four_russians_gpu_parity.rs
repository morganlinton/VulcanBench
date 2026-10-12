//! Parity test: vyre-primitives four_russians_apply_byte_lut matches CPU oracle.

#![cfg(test)]

mod common;

use common::{bytes_u32, u32_bytes, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::bitset::four_russians::{
    binary_byte_lut, cpu_ref, four_russians_apply_byte_lut, BooleanTileOp,
};

fn run(lhs: &[u32], rhs: &[u32], lut: &[u32]) -> Vec<u32> {
    let words = lhs.len().min(rhs.len()) as u32;
    let program = four_russians_apply_byte_lut("lhs", "rhs", "lut", "out", words);
    let inputs: Vec<Vec<u8>> = vec![
        u32_bytes(lhs),
        u32_bytes(rhs),
        u32_bytes(lut),
        // out: zero-init.
        vec![0u8; words as usize * 4],
    ];
    let mut config = DispatchConfig::default();
    let workgroup_x = 256u32;
    let grid_x = ((words + workgroup_x - 1) / workgroup_x).max(1);
    config.grid_override = Some([grid_x, 1, 1]);
    let outputs = with_live_backend("four-russians byte LUT", |backend| {
        backend
            .dispatch(&program, &inputs, &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA Four Russians dispatch failed: {error}"))
    });
    let mut out = bytes_u32(&outputs[0]);
    out.truncate(words as usize);
    out
}

#[test]
fn cuda_four_russians_and_op() {
    let lut = binary_byte_lut(BooleanTileOp::And);
    let lhs = vec![0xFF00_FF00u32, 0x0F0F_0F0F];
    let rhs = vec![0xF0F0_F0F0u32, 0xFFFF_0000];
    let cpu = cpu_ref(&lhs, &rhs, &lut);
    let gpu = run(&lhs, &rhs, &lut);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_four_russians_or_op() {
    let lut = binary_byte_lut(BooleanTileOp::Or);
    let lhs = vec![0xAAAA_AAAAu32, 0x5555_5555];
    let rhs = vec![0x5555_5555u32, 0xAAAA_AAAA];
    let cpu = cpu_ref(&lhs, &rhs, &lut);
    let gpu = run(&lhs, &rhs, &lut);
    assert_eq!(gpu, cpu);
    // every bit set
    assert_eq!(gpu, vec![0xFFFF_FFFFu32, 0xFFFF_FFFF]);
}

#[test]
fn cuda_four_russians_xor_op() {
    let lut = binary_byte_lut(BooleanTileOp::Xor);
    let lhs = vec![0xCAFE_CAFEu32];
    let rhs = vec![0xBABE_BABEu32];
    let cpu = cpu_ref(&lhs, &rhs, &lut);
    let gpu = run(&lhs, &rhs, &lut);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_four_russians_andnot_op() {
    let lut = binary_byte_lut(BooleanTileOp::AndNot);
    let lhs = vec![0xDEAD_BEEFu32, 0xFEED_FACE];
    let rhs = vec![0xFFFF_0000u32, 0x00FF_FF00];
    let cpu = cpu_ref(&lhs, &rhs, &lut);
    let gpu = run(&lhs, &rhs, &lut);
    assert_eq!(gpu, cpu);
}

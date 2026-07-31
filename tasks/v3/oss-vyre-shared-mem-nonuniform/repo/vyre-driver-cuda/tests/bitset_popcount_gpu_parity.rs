//! Parity test: GPU per-word popcount matches CPU oracle.
//!
//! Drives `vyre_self_substrate::bitset_summary::per_word_popcount_via`
//! against the CPU oracle on real CUDA hardware. Asserts identical
//! per-word popcount across a battery of inputs.

#![cfg(test)]

mod common;

use common::with_cuda_optimizer_dispatcher;
use vyre_self_substrate::bitset_summary::{
    per_word_popcount, per_word_popcount_via, saturation_ratio, saturation_ratio_via,
    total_set_bits, total_set_bits_via,
};

#[test]
fn cuda_per_word_popcount_matches_cpu_small() {
    let input = vec![0u32, 1, 0xFFFF_FFFF, 0xAAAA_AAAA, 0x12345678];
    let gpu = with_cuda_optimizer_dispatcher("small popcount", |dispatcher| {
        per_word_popcount_via(dispatcher, &input).expect("dispatch")
    });
    let cpu = per_word_popcount(&input);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_per_word_popcount_handles_empty() {
    let gpu = with_cuda_optimizer_dispatcher("empty popcount", |dispatcher| {
        per_word_popcount_via(dispatcher, &[]).expect("dispatch")
    });
    assert!(gpu.is_empty());
}

#[test]
fn cuda_per_word_popcount_large_input() {
    // 1024 words covers multiple workgroups (workgroup_size=256).
    let input: Vec<u32> = (0..1024).map(|i| i as u32 ^ 0xDEAD_BEEF).collect();
    let gpu = with_cuda_optimizer_dispatcher("large popcount", |dispatcher| {
        per_word_popcount_via(dispatcher, &input).expect("dispatch")
    });
    let cpu = per_word_popcount(&input);
    assert_eq!(gpu, cpu, "popcount divergence on n=1024");
}

#[test]
fn cuda_total_set_bits_via_matches_cpu() {
    let input: Vec<u32> = (0..512)
        .map(|i| (i as u32).wrapping_mul(0x9E37_79B9))
        .collect();
    let gpu = with_cuda_optimizer_dispatcher("total set bits", |dispatcher| {
        total_set_bits_via(dispatcher, &input).expect("dispatch")
    });
    let cpu = total_set_bits(&input);
    assert_eq!(gpu, cpu, "total_set_bits divergence: gpu={gpu} cpu={cpu}");
}

#[test]
fn cuda_saturation_ratio_via_matches_cpu() {
    let input = vec![0xAAAA_AAAAu32; 64]; // 50% saturation across 64 words.
    let gpu = with_cuda_optimizer_dispatcher("saturation ratio", |dispatcher| {
        saturation_ratio_via(dispatcher, &input).expect("dispatch")
    });
    let cpu = saturation_ratio(&input);
    assert!(
        (gpu - cpu).abs() < 1e-9,
        "saturation_ratio divergence: gpu={gpu} cpu={cpu}"
    );
}

#[test]
fn cuda_per_word_popcount_partial_workgroup() {
    // 200 words  -  fewer than one workgroup; tests bounds-check.
    let input: Vec<u32> = (0..200)
        .map(|i| (i as u32).wrapping_mul(0x9E37_79B9))
        .collect();
    let gpu = with_cuda_optimizer_dispatcher("partial workgroup popcount", |dispatcher| {
        per_word_popcount_via(dispatcher, &input).expect("dispatch")
    });
    let cpu = per_word_popcount(&input);
    assert_eq!(gpu, cpu);
}

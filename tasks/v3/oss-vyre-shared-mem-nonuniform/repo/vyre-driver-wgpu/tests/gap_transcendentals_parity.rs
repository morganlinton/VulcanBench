//! Release gap #1 - cross-backend bitwise transcendental parity.
//!
//! See `contracts/release.md`. WGSL hardware transcendentals do not
//! guarantee correctly-rounded f32 today, so this test **fails** until
//! vyre emits deterministic transcendental expansions that match
//! `vyre_reference::ieee754::canonical_*` bit-for-bit. The bounded-ULP
//! envelope shipped in `transcendentals_parity.rs` is the interim gate;
//! this file is the aspirational bitwise contract that closes gap #1.
#![cfg(feature = "parity-testing")]

use proptest::prelude::*;
use std::sync::OnceLock;
use vyre::ir::UnOp;
use vyre_driver_wgpu::WgpuBackend;

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| {
        WgpuBackend::acquire()
            .expect("Fix: gap_transcendentals_parity requires a local GPU-backed wgpu backend")
    })
}

fn gpu_unary_many(backend: &WgpuBackend, op: UnOp, xs: &[f32]) -> Vec<f32> {
    backend
        .probe_op_many(op, xs)
        .expect("Fix: wgpu f32 unary batch probe must dispatch successfully")
}

fn cpu_canonical(op: &UnOp, x: f32) -> f32 {
    use vyre_reference::ieee754::{
        canonical_cos, canonical_exp, canonical_log, canonical_sin, canonical_sqrt,
    };
    match op {
        UnOp::Sin => canonical_sin(x),
        UnOp::Cos => canonical_cos(x),
        UnOp::Sqrt => canonical_sqrt(x),
        UnOp::Exp => canonical_exp(x),
        UnOp::Log => canonical_log(x),
        other => panic!(
            "Fix: gap_transcendentals_parity only covers sin/cos/sqrt/exp/log, got {other:?}"
        ),
    }
}

fn assert_bitwise_parity(op: UnOp, x: f32, gpu: f32) {
    let cpu = cpu_canonical(&op, x);
    assert_eq!(
        cpu.to_bits(),
        gpu.to_bits(),
        "gap_transcendentals_parity: {op:?}({x}) cpu={cpu} ({:#010x}) vs gpu={gpu} ({:#010x}) \
         must be byte-identical per contracts/release.md gap #1",
        cpu.to_bits(),
        gpu.to_bits()
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn sin_bitwise_parity(xs in prop::collection::vec(-10.0f32..10.0f32, 1..=8)) {
        let gpu = gpu_unary_many(backend(), UnOp::Sin, &xs);
        for (x, gpu) in xs.into_iter().zip(gpu) {
            assert_bitwise_parity(UnOp::Sin, x, gpu);
        }
    }

    #[test]
    fn cos_bitwise_parity(xs in prop::collection::vec(-10.0f32..10.0f32, 1..=8)) {
        let gpu = gpu_unary_many(backend(), UnOp::Cos, &xs);
        for (x, gpu) in xs.into_iter().zip(gpu) {
            assert_bitwise_parity(UnOp::Cos, x, gpu);
        }
    }

    #[test]
    fn sqrt_bitwise_parity(xs in prop::collection::vec(0.0f32..10.0f32, 1..=8)) {
        let gpu = gpu_unary_many(backend(), UnOp::Sqrt, &xs);
        for (x, gpu) in xs.into_iter().zip(gpu) {
            assert_bitwise_parity(UnOp::Sqrt, x, gpu);
        }
    }

    #[test]
    fn exp_bitwise_parity(xs in prop::collection::vec(-10.0f32..10.0f32, 1..=8)) {
        let gpu = gpu_unary_many(backend(), UnOp::Exp, &xs);
        for (x, gpu) in xs.into_iter().zip(gpu) {
            assert_bitwise_parity(UnOp::Exp, x, gpu);
        }
    }

    #[test]
    fn log_bitwise_parity(xs in prop::collection::vec(0.000_001f32..10.0f32, 1..=8)) {
        let gpu = gpu_unary_many(backend(), UnOp::Log, &xs);
        for (x, gpu) in xs.into_iter().zip(gpu) {
            assert_bitwise_parity(UnOp::Log, x, gpu);
        }
    }
}

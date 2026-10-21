//! Parity for the u32 mulhi / abs_diff / saturating / rotate binops against
//! Rust/oracle on the live CUDA device — the PTX/CUDA twin of the wgpu
//! `synthetic_binop_parity` gate.
//!
//! These ops are the class the naga signed-`Modulo` miscompile proved can be
//! silently wrong on real silicon, and the PTX backend lowers them DIFFERENTLY
//! from naga's multi-step `vec2`/`select` synthesis (PTX has native `mul.hi.u32`,
//! `vabsdiff`, funnel-shift `shf`, and `*.sat` forms), so naga's GPU-locked
//! result transfers NOTHING — the PTX route needs its own live-GPU proof. No
//! CUDA test exercised mulhi / abs_diff / saturating_{add,sub,mul} /
//! rotate_{left,right} directly with overflow/edge operands; a wrong native
//! instruction, an unmasked rotate amount, or a missing saturate clamp would be
//! invisible.
//!
//! Each op is dispatched on the 5090 over overflow/identity-boundary operands
//! (0, 1, u32::MAX, 2^31, oversized rotate amounts) and asserted byte-for-byte
//! against the Rust std reference, which IS the oracle contract.

mod common;
use common::live_backend;

use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Little-endian `u32` words -> bytes (self-contained).
fn u32_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &w in words {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    bytes
}

fn program(n: u32, build: fn(Expr, Expr) -> Expr) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            build(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn dispatch(backend: &CudaBackend, program: &Program, ps: &[(u32, u32)]) -> Vec<u32> {
    let a = u32_bytes(&ps.iter().map(|&(a, _)| a).collect::<Vec<_>>());
    let b = u32_bytes(&ps.iter().map(|&(_, b)| b).collect::<Vec<_>>());
    let out_init = u32_bytes(&vec![0u32; ps.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA must dispatch the synthetic-binop parity contract.");
    assert_eq!(
        outputs.len(),
        1,
        "program declares one ReadWrite output; CUDA returned {} buffer(s)",
        outputs.len()
    );
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Dispatch `build` on `pairs` and assert byte-for-byte against `reference`.
fn check(
    backend: &CudaBackend,
    build: fn(Expr, Expr) -> Expr,
    reference: fn(u32, u32) -> u32,
    pairs: &[(u32, u32)],
    name: &str,
) {
    let gpu = dispatch(backend, &program(pairs.len() as u32, build), pairs);
    let expected: Vec<u32> = pairs.iter().map(|&(a, b)| reference(a, b)).collect();
    assert_eq!(
        gpu, expected,
        "CUDA `{name}` diverged from the Rust/oracle reference (the PTX lowering \
         miscompiles on hardware).\n  pairs:    {pairs:?}\n  \
         expected: {expected:?}\n  gpu:      {gpu:?}"
    );
}

/// Operands spanning the overflow/identity boundaries every op cares about:
/// zero, one, the max, the sign bit, and mid-range values.
fn extremes() -> Vec<(u32, u32)> {
    vec![
        (0, 0),
        (1, 1),
        (u32::MAX, 1),
        (1, u32::MAX),
        (u32::MAX, u32::MAX),
        (0x8000_0000, 0x8000_0000),
        (0x1_0000, 0x1_0000),
        (100, 50),
        (50, 100),
        (0x7FFF_FFFF, 2),
    ]
}

#[test]
fn mulhi_matches_widening_high_word_on_cuda() {
    let backend = live_backend();
    let reference = |a: u32, b: u32| ((u64::from(a) * u64::from(b)) >> 32) as u32;
    check(&backend, Expr::mulhi, reference, &extremes(), "mulhi");
    assert_eq!(reference(u32::MAX, u32::MAX), 0xFFFF_FFFE);
    assert_eq!(reference(0x1_0000, 0x1_0000), 1);
}

#[test]
fn abs_diff_matches_unsigned_absolute_difference_on_cuda() {
    let backend = live_backend();
    check(&backend, Expr::abs_diff, u32::abs_diff, &extremes(), "abs_diff");
    assert_eq!(u32::abs_diff(0, u32::MAX), u32::MAX);
    assert_eq!(u32::abs_diff(100, 50), 50);
}

#[test]
fn saturating_add_clamps_to_max_on_cuda() {
    let backend = live_backend();
    check(
        &backend,
        Expr::saturating_add,
        u32::saturating_add,
        &extremes(),
        "saturating_add",
    );
    assert_eq!(u32::saturating_add(u32::MAX, 1), u32::MAX);
    assert_eq!(u32::saturating_add(0x8000_0000, 0x8000_0000), u32::MAX);
}

#[test]
fn saturating_sub_clamps_to_zero_on_cuda() {
    let backend = live_backend();
    check(
        &backend,
        Expr::saturating_sub,
        u32::saturating_sub,
        &extremes(),
        "saturating_sub",
    );
    assert_eq!(u32::saturating_sub(1, u32::MAX), 0);
    assert_eq!(u32::saturating_sub(100, 50), 50);
}

#[test]
fn saturating_mul_clamps_to_max_on_cuda() {
    let backend = live_backend();
    let mut pairs = extremes();
    pairs.extend_from_slice(&[(0x1_0000, 0x1_0000), (0x8000, 0x2_0000), (1000, 1000), (3, 4)]);
    check(&backend, Expr::saturating_mul, u32::saturating_mul, &pairs, "saturating_mul");
    assert_eq!(u32::saturating_mul(0x1_0000, 0x1_0000), u32::MAX); // 2^32 overflows
    assert_eq!(u32::saturating_mul(1000, 1000), 1_000_000);
}

/// Value/amount pairs spanning the rotate-mask boundary (0, mid, 31, 32, 33).
fn rotate_pairs() -> Vec<(u32, u32)> {
    vec![
        (1, 0),
        (1, 1),
        (1, 31),
        (1, 32),
        (1, 33),
        (0x8000_0000, 1),
        (0xDEAD_BEEF, 4),
        (0xDEAD_BEEF, 8),
        (0xDEAD_BEEF, 16),
        (0xFFFF_FFFF, 17),
    ]
}

#[test]
fn rotate_left_matches_barrel_rotate_on_cuda() {
    let backend = live_backend();
    let reference = |a: u32, b: u32| a.rotate_left(b & 31);
    check(&backend, Expr::rotate_left, reference, &rotate_pairs(), "rotate_left");
    assert_eq!(reference(1, 32), 1); // 1<<32 rotate == identity (mask)
    assert_eq!(reference(0x8000_0000, 1), 1); // wrap
    assert_eq!(reference(0xDEAD_BEEF, 4), 0xEADB_EEFD);
}

#[test]
fn rotate_right_matches_barrel_rotate_on_cuda() {
    let backend = live_backend();
    let reference = |a: u32, b: u32| a.rotate_right(b & 31);
    check(&backend, Expr::rotate_right, reference, &rotate_pairs(), "rotate_right");
    assert_eq!(reference(1, 1), 0x8000_0000);
    assert_eq!(reference(1, 32), 1);
    assert_eq!(reference(0xDEAD_BEEF, 4), 0xFDEA_DBEE);
}

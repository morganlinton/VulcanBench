//! Parity for the SYNTHETIC u32 binop lowerings against Rust/oracle on the
//! live GPU.
//!
//! naga has no native instruction for these ops; op_dispatch synthesizes each
//! from a multi-step expression — the exact class of computed lowering the naga
//! signed-`Modulo` bug proved can be silently wrong on real silicon:
//!   * `mulhi`         -> 16-bit decomposition (al*bl + cross terms + ah*bh)
//!   * `abs_diff`      -> `select(a < b, b - a, a - b)`
//!   * `saturating_add`-> `select(a + b < a, MAX, a + b)`
//!   * `saturating_sub`-> `select(a < b, 0, a - b)`
//!   * `saturating_mul`-> `select(b != 0 && a > MAX/b, MAX, a * b)`
//!   * `rotate_left/right` -> `(x << (s&31)) | (x >> ((32-(s&31))&31))`
//!
//! Rotate is exercised inside the real BLAKE3 workload by
//! `blake3_compress_gpu_parity`; this isolates every synthetic op directly with
//! overflow/edge operands (0, 1, u32::MAX, 2^31, oversized rotate amounts) and
//! asserts byte-for-byte against the Rust std reference, which IS the oracle
//! contract (`saturating_add`, `abs_diff`, `rotate_left`, widening `mulhi`).

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

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

fn dispatch(backend: &WgpuBackend, program: &Program, ps: &[(u32, u32)]) -> Vec<u32> {
    let a = u32_bytes(&ps.iter().map(|&(a, _)| a).collect::<Vec<_>>());
    let b = u32_bytes(&ps.iter().map(|&(_, b)| b).collect::<Vec<_>>());
    let out_init = u32_bytes(&vec![0u32; ps.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the synthetic-binop parity contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Dispatch `build` on `pairs` and assert byte-for-byte against `reference`.
fn check(
    backend: &WgpuBackend,
    build: fn(Expr, Expr) -> Expr,
    reference: fn(u32, u32) -> u32,
    pairs: &[(u32, u32)],
    name: &str,
) {
    let gpu = dispatch(backend, &program(pairs.len() as u32, build), pairs);
    let expected: Vec<u32> = pairs.iter().map(|&(a, b)| reference(a, b)).collect();
    assert_eq!(
        gpu, expected,
        "GPU synthetic `{name}` diverged from the Rust/oracle reference (the \
         multi-step lowering miscompiles on hardware).\n  pairs:    {pairs:?}\n  \
         expected: {expected:?}\n  gpu:      {gpu:?}"
    );
}

/// Operands spanning the overflow/identity boundaries every synthetic op cares
/// about: zero, one, the max, the sign bit, and mid-range values.
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
fn mulhi_matches_widening_high_word_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
    let reference = |a: u32, b: u32| ((u64::from(a) * u64::from(b)) >> 32) as u32;
    check(&backend, Expr::mulhi, reference, &extremes(), "mulhi");
    // Pin the load-bearing cases literally: MAX*MAX high word, 2^16 squared.
    assert_eq!(reference(u32::MAX, u32::MAX), 0xFFFF_FFFE);
    assert_eq!(reference(0x1_0000, 0x1_0000), 1);
}

#[test]
fn abs_diff_matches_unsigned_absolute_difference_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
    check(&backend, Expr::abs_diff, u32::abs_diff, &extremes(), "abs_diff");
    assert_eq!(u32::abs_diff(0, u32::MAX), u32::MAX);
    assert_eq!(u32::abs_diff(100, 50), 50);
}

#[test]
fn saturating_add_clamps_to_max_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
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
fn saturating_sub_clamps_to_zero_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
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
fn saturating_mul_clamps_to_max_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
    // Add multiplicative-overflow pairs on top of the shared extremes.
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
fn rotate_left_matches_barrel_rotate_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
    let reference = |a: u32, b: u32| a.rotate_left(b & 31);
    check(&backend, Expr::rotate_left, reference, &rotate_pairs(), "rotate_left");
    // 1<<32 rotate == identity (mask), 0x80000000 rotl 1 == 1 (wrap).
    assert_eq!(reference(1, 32), 1);
    assert_eq!(reference(0x8000_0000, 1), 1);
    assert_eq!(reference(0xDEAD_BEEF, 4), 0xEADB_EEFD);
}

#[test]
fn rotate_right_matches_barrel_rotate_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: synthetic-binop parity needs a live GPU.");
    let reference = |a: u32, b: u32| a.rotate_right(b & 31);
    check(&backend, Expr::rotate_right, reference, &rotate_pairs(), "rotate_right");
    assert_eq!(reference(1, 1), 0x8000_0000);
    assert_eq!(reference(1, 32), 1);
    assert_eq!(reference(0xDEAD_BEEF, 4), 0xFDEA_DBEE);
}

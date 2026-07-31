//! Integer unary-op parity against the reference oracle on the live GPU.
//!
//! `transcendentals_parity` covers the FLOAT unary ops (sin/cos/sqrt/...), but
//! the INTEGER unary ops were never GPU-parity-tested. Two map to native naga
//! operators (`Negate`, `BitwiseNot`) and four to Math intrinsics
//! (`CountOneBits`, `CountLeadingZeros`, `CountTrailingZeros`, `ReverseBits`).
//! The naga signed-`Modulo` bug proved that even a native/intrinsic naga op can
//! silently pick the wrong semantics on hardware, so these are dispatched on the
//! 5090 and asserted byte-for-byte against the oracle contract:
//!   * `negate(u32)` = `0u32.wrapping_sub(v)` = `v.wrapping_neg()`. NOTE: vyre's
//!     typecheck legalises integer `Negate` only for `u32` (and `f32`); RAW
//!     `i32` negate is REJECTED upstream precisely because of the `i32::MIN`
//!     overflow case (it routes users to `0 - x` / cast-to-u32). So the integer
//!     negate that reaches the GPU is the wrapping unsigned one — and WGSL has no
//!     native unary minus on `u32`, so this also proves the emitter lowers it
//!     correctly (not a naga-rejected `-(u32)`).
//!   * `bitnot(u32)` = `!v`.
//!   * `popcount(u32)` = `v.count_ones()`.
//!   * `clz(u32)` = `v.leading_zeros()`  — edge `clz(0) == 32`.
//!   * `ctz(u32)` = `v.trailing_zeros()` — edge `ctz(0) == 32`.
//!   * `reverse_bits(u32)` = `v.reverse_bits()`.
//!
//! (Integer `abs`/`sign` are intentionally absent: the oracle's integer unary
//! dispatch errors on them — they are float-only ops by contract.)

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// `out[i] = build(load(a, i))` over single-input buffers of element `elem`.
fn unary_program(elem: DataType, n: u32, build: fn(Expr) -> Expr) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store("out", Expr::u32(i), build(Expr::load("a", Expr::u32(i)))));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, elem.clone()).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, elem).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn dispatch(backend: &WgpuBackend, program: &Program, input_words: &[u32]) -> Vec<u32> {
    let a = u32_bytes(input_words);
    let out_init = u32_bytes(&vec![0u32; input_words.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), a.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the integer unary-op contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn negate_u32_wraps_two_complement_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    // u32 bit patterns incl the sign bit and the extremes.
    let ins: [u32; 8] = [0, 1, 0xFFFF_FFFF, 0x8000_0000, 2, 0x7FFF_FFFF, 100, 0xDEAD_BEEF];
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::negate), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| v.wrapping_neg()).collect();
    // Pin the contract: negate(1) wraps to u32::MAX, negate(2^31) is itself.
    assert_eq!(
        expected,
        vec![0, 0xFFFF_FFFF, 1, 0x8000_0000, 0xFFFF_FFFE, 0x8000_0001, 0xFFFF_FF9C, 0x2152_4111]
    );
    assert_eq!(
        gpu, expected,
        "GPU negate(u32) diverged from wrapping `0 - v` (the emitter must lower \
         unsigned unary minus, which WGSL lacks natively).\n  \
         inputs: {ins:08x?}\n  expected: {expected:08x?}\n  gpu: {gpu:08x?}"
    );
}

/// u32 bit patterns spanning the count/reverse edges: zero, one, all-ones, the
/// sign bit, alternating, and a low byte.
fn bit_inputs() -> Vec<u32> {
    vec![0, 1, 0xFFFF_FFFF, 0x8000_0000, 0xAAAA_AAAA, 0x5555_5555, 0x0000_00FF, 0xDEAD_BEEF]
}

#[test]
fn bitnot_u32_complements_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    let ins = bit_inputs();
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::bitnot), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| !v).collect();
    assert_eq!(gpu, expected, "GPU bitnot(u32) diverged from `!v`.\n  inputs: {ins:08x?}\n  gpu: {gpu:08x?}");
}

#[test]
fn popcount_u32_counts_one_bits_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    let ins = bit_inputs();
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::popcount), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| v.count_ones()).collect();
    assert_eq!(expected, vec![0, 1, 32, 1, 16, 16, 8, 24], "popcount reference drifted");
    assert_eq!(gpu, expected, "GPU popcount(u32) diverged from count_ones.\n  inputs: {ins:08x?}\n  gpu: {gpu:?}");
}

#[test]
fn clz_u32_counts_leading_zeros_incl_zero_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    let ins = bit_inputs();
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::clz), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| v.leading_zeros()).collect();
    // Edge: clz(0) == 32 (whole word), clz(1) == 31, clz(0x80000000) == 0.
    assert_eq!(expected, vec![32, 31, 0, 0, 0, 1, 24, 0], "clz reference drifted");
    assert_eq!(gpu, expected, "GPU clz(u32) diverged from leading_zeros (clz(0) must be 32).\n  inputs: {ins:08x?}\n  gpu: {gpu:?}");
}

#[test]
fn ctz_u32_counts_trailing_zeros_incl_zero_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    let ins = bit_inputs();
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::ctz), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| v.trailing_zeros()).collect();
    // Edge: ctz(0) == 32, ctz(0x80000000) == 31, ctz(0xFF) == 0.
    assert_eq!(expected, vec![32, 0, 0, 31, 1, 0, 0, 0], "ctz reference drifted");
    assert_eq!(gpu, expected, "GPU ctz(u32) diverged from trailing_zeros (ctz(0) must be 32).\n  inputs: {ins:08x?}\n  gpu: {gpu:?}");
}

#[test]
fn reverse_bits_u32_matches_on_gpu() {
    let backend = WgpuBackend::acquire().expect("Fix: unary int parity needs a live GPU.");
    let ins = bit_inputs();
    let gpu = dispatch(&backend, &unary_program(DataType::U32, ins.len() as u32, Expr::reverse_bits), &ins);
    let expected: Vec<u32> = ins.iter().map(|&v| v.reverse_bits()).collect();
    assert_eq!(expected[1], 0x8000_0000, "reverse_bits(1) must be the sign bit");
    assert_eq!(gpu, expected, "GPU reverse_bits(u32) diverged.\n  inputs: {ins:08x?}\n  expected: {expected:08x?}\n  gpu: {gpu:08x?}");
}

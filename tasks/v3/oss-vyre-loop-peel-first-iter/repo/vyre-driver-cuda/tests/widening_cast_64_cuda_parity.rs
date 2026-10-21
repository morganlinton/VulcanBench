//! 64-bit widening-cast (`i32`/`u32` -> `i64`/`u64`) parity against Rust `as` on
//! the live CUDA device — the PTX/CUDA twin of the wgpu
//! `widening_cast_64_parity` gate.
//!
//! The wgpu/naga path lowers `U64`/`I64` as `vec2<u32>` and synthesizes the high
//! word from the SOURCE's signedness; that path is GPU-locked. The PTX backend
//! takes a DIFFERENT route: it keeps the 64-bit value in a native 64-bit register
//! and emits the hardware widening convert — `cvt.s64.s32` for a signed source
//! (sign-extend, `emit_cast` scalar.rs `(I32,U64)` arm) and `cvt.u64.u32` for an
//! unsigned source (zero-extend, the `(U32,U64)` arm); `I64` reaches these via
//! `from_dtype(I64) -> U64`. That PTX route's correctness was concluded ONLY by a
//! SOURCE READ — exactly the class the naga signed-`Modulo` miscompile proved can
//! be silently wrong on real silicon (a source read is NOT proof). No CUDA test
//! had ever dispatched a 64-bit-output buffer, so a wrong `cvt`, a mis-sized
//! 8-byte element store, or a high-word leak would be invisible.
//!
//! These tests dispatch the cast on the 5090, read back BOTH 32-bit words of each
//! 64-bit element, and assert the full `u64` byte-for-byte against Rust `as`. The
//! Law-10 case is a NEGATIVE `i32` widened into an UNSIGNED `u64`: it must STILL
//! sign-extend (`-7i32 as u64 == 0xFFFF_FFFF_FFFF_FFF9`), driven by the SOURCE
//! signedness, not the target's.

mod common;
use common::live_backend;

use vyre::DispatchConfig;
use vyre_driver_cuda::CudaBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Little-endian `u32` words -> bytes (self-contained; no dependency on the
/// shared matrix helpers' word-packing).
fn u32_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &w in words {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    bytes
}

/// `out[i] = cast(target64, load(src, i))` — `src` is a 32-bit buffer (signed or
/// unsigned per `src_ty`), `out` is the 64-bit `target64` buffer (8 bytes/elem).
/// One thread ([1,1,1]) writes every element, mirroring the wgpu gate so the
/// only moving part is the backend.
fn widen_program(src_ty: DataType, target64: DataType, n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::cast(target64.clone(), Expr::load("src", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, target64).with_count(n),
            BufferDecl::storage("src", 1, BufferAccess::ReadOnly, src_ty).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

/// Dispatch and reconstruct each 64-bit element from its two little-endian words.
fn run(backend: &CudaBackend, src_ty: DataType, target64: DataType, words: &[u32]) -> Vec<u64> {
    let n = words.len() as u32;
    let program = widen_program(src_ty, target64, n);
    let src_bytes = u32_bytes(words);
    // 8 bytes per 64-bit output element => 2 zero u32 words each.
    let out_init = u32_bytes(&vec![0u32; words.len() * 2]);
    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[out_init.as_slice(), src_bytes.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA must dispatch the 64-bit widening-cast contract.");
    assert_eq!(
        outputs.len(),
        1,
        "widening program declares one ReadWrite output (out); CUDA returned {} buffer(s)",
        outputs.len()
    );
    assert_eq!(
        outputs[0].len(),
        words.len() * 8,
        "CUDA 64-bit output buffer must be 8 bytes per element; got {} bytes for {} elements",
        outputs[0].len(),
        words.len()
    );
    outputs[0]
        .chunks_exact(8)
        .map(|c| {
            let low = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            let high = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
            (u64::from(high) << 32) | u64::from(low)
        })
        .collect()
}

/// Bit patterns spanning the sign boundary and the extremes.
fn signed_inputs() -> Vec<i32> {
    vec![-7, 7, -1, 0, 1, i32::MIN, i32::MAX, -128, 0x4000_0000, -0x4000_0000]
}

#[test]
fn i32_to_i64_sign_extends_high_word_on_cuda() {
    let backend = live_backend();
    let ins = signed_inputs();
    let words: Vec<u32> = ins.iter().map(|&v| v as u32).collect();
    let gpu = run(&backend, DataType::I32, DataType::I64, &words);
    let expected: Vec<u64> = ins.iter().map(|&v| (v as i64) as u64).collect();
    // Pin the contract literally: negatives MUST carry a 0xFFFF_FFFF high word.
    assert_eq!(
        expected,
        vec![
            0xFFFF_FFFF_FFFF_FFF9,
            0x0000_0000_0000_0007,
            0xFFFF_FFFF_FFFF_FFFF,
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0xFFFF_FFFF_8000_0000,
            0x0000_0000_7FFF_FFFF,
            0xFFFF_FFFF_FFFF_FF80,
            0x0000_0000_4000_0000,
            0xFFFF_FFFF_C000_0000,
        ],
        "reference i32->i64 sign-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA i32->i64 widening diverged from `as i64` (cvt.s64.s32 high-word \
         sign-replicate regressed; negative source zero-extended).\n  inputs:   {ins:?}\n  \
         expected: {expected:#018x?}\n  gpu:      {gpu:#018x?}"
    );
}

#[test]
fn i32_to_u64_sign_extends_high_word_on_cuda() {
    // The Law 10 case: a NEGATIVE signed source widened into an UNSIGNED 64-bit
    // target still sign-extends (`-7i32 as u64 == 0xFFFF_FFFF_FFFF_FFF9`, NOT
    // 0x0000_0000_FFFF_FFF9). The target's unsignedness does not change the
    // SOURCE-driven high word.
    let backend = live_backend();
    let ins = signed_inputs();
    let words: Vec<u32> = ins.iter().map(|&v| v as u32).collect();
    let gpu = run(&backend, DataType::I32, DataType::U64, &words);
    let expected: Vec<u64> = ins.iter().map(|&v| v as u64).collect();
    assert_eq!(
        expected[0], 0xFFFF_FFFF_FFFF_FFF9,
        "reference i32->u64 sign-extension drifted (-7 must carry the high word)"
    );
    assert_eq!(
        gpu, expected,
        "CUDA i32->u64 widening diverged from `as u64` (negative source must STILL \
         sign-extend into an unsigned target).\n  inputs:   {ins:?}\n  \
         expected: {expected:#018x?}\n  gpu:      {gpu:#018x?}"
    );
}

#[test]
fn u32_to_u64_zero_extends_high_word_on_cuda() {
    // The twin: an UNSIGNED source ZERO-extends. 0xFFFF_FFFF as u64 must be
    // 0x0000_0000_FFFF_FFFF, NOT sign-extended — proves the signedness gate keys
    // on the SOURCE, not the value's top bit.
    let backend = live_backend();
    let words: Vec<u32> = vec![0xFFFF_FFFF, 0x8000_0000, 7, 0, 1, 0x7FFF_FFFF, 0xDEAD_BEEF];
    let gpu = run(&backend, DataType::U32, DataType::U64, &words);
    let expected: Vec<u64> = words.iter().map(|&w| u64::from(w)).collect();
    assert_eq!(
        expected[0], 0x0000_0000_FFFF_FFFF,
        "reference u32->u64 zero-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->u64 widening diverged from `as u64` (unsigned source must \
         ZERO-extend; high word leaked).\n  inputs:   {words:#010x?}\n  \
         expected: {expected:#018x?}\n  gpu:      {gpu:#018x?}"
    );
}

#[test]
fn u32_to_i64_zero_extends_into_signed_target_on_cuda() {
    // An UNSIGNED source widened into a SIGNED 64-bit target ZERO-extends —
    // `0xFFFF_FFFFu32 as i64 == 0x0000_0000_FFFF_FFFF` (4294967295), not -1. The
    // PTX route reaches I64 via `from_dtype(I64) -> U64`, so this must select the
    // `cvt.u64.u32` zero-extend arm off the SOURCE, not the I64 target name.
    let backend = live_backend();
    let words: Vec<u32> = vec![0xFFFF_FFFF, 0x8000_0000, 7, 0, 0x7FFF_FFFF];
    let gpu = run(&backend, DataType::U32, DataType::I64, &words);
    let expected: Vec<u64> = words.iter().map(|&w| (u64::from(w) as i64) as u64).collect();
    assert_eq!(
        expected[0], 0x0000_0000_FFFF_FFFF,
        "reference u32->i64 zero-extension drifted (0xFFFFFFFF must be +4294967295, not -1)"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->i64 widening diverged from `as i64` (unsigned source must \
         ZERO-extend even into a signed 64-bit target).\n  inputs:   {words:#010x?}\n  \
         expected: {expected:#018x?}\n  gpu:      {gpu:#018x?}"
    );
}

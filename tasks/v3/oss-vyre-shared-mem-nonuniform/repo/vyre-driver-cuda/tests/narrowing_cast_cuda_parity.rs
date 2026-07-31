//! Integer-narrowing cast (`u32` -> `u8`/`u16`/`i8`/`i16`) parity against Rust
//! `as` / the reference oracle on the live CUDA device — the PTX/CUDA twin of the
//! wgpu `narrowing_cast_parity` gate.
//!
//! A narrowing cast is validate-LEGAL (V035 only WARNS), so it reaches the GPU.
//! The PTX backend's `from_dtype` COLLAPSES U8/U16->U32 and I8/I16->I32, so a bare
//! convert is a no-op for a same-width source; the narrowing fix made `emit_cast`
//! emit the canonical `cvt.u32.u8` (zero-extend) / `cvt.s32.s8` (sign-extend)
//! BEFORE the identity early-return. That PTX path was unit-asserted but NEVER
//! dispatched on a live CUDA device — the same source-read-vs-hardware gap the
//! naga signed-`Modulo` miscompile punished. If the `cvt` were skipped, `300u32 as
//! u8` would read back 300 instead of 44 (a silent non-narrowing divergence).
//!
//! To isolate the CAST's narrowing from the byte-element STORE (which masks to a
//! byte regardless), the narrowed value is widened back out — `cast(WIDE,
//! cast(NARROW, x))` — and stored into a 32-bit buffer, so the word read back
//! reflects exactly what the narrowing cast produced.

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

/// Probe inputs (u32 bit patterns) exercising truncation and the signed boundary:
/// 300 (low byte 44), 0x12345 (low half 0x2345), 200 (i8 -56), 0xFFFF (i16 -1 /
/// u16 max), 0x8000 (i16 MIN), 0xFFFFFFFF (all ones), 0, 127, 128, 255.
fn inputs() -> Vec<u32> {
    vec![
        300, 0x0001_2345, 200, 0x0000_FFFF, 0x0000_8000, 0xFFFF_FFFF, 0, 127, 128, 255,
    ]
}

/// `out = cast(wide, cast(narrow, load(input)))` for every input word. `wide` is
/// the non-narrowing integer that round-trips the narrowed value into a 32-bit
/// store slot (U32 for U8/U16, I32 for I8/I16).
fn narrow_program(narrow: DataType, wide: DataType, n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::cast(
                wide.clone(),
                Expr::cast(narrow.clone(), Expr::load("input", Expr::u32(i))),
            ),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, wide).with_count(n),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn run(backend: &CudaBackend, narrow: DataType, wide: DataType) -> Vec<u32> {
    let ins = inputs();
    let n = ins.len() as u32;
    let program = narrow_program(narrow, wide, n);
    let input_bytes = u32_bytes(&ins);
    let out_init = u32_bytes(&vec![0u32; ins.len()]);
    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[out_init.as_slice(), input_bytes.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA must dispatch the narrowing-cast parity contract.");
    assert_eq!(
        outputs.len(),
        1,
        "narrowing program declares one ReadWrite output; CUDA returned {} buffer(s)",
        outputs.len()
    );
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn u32_to_u8_narrowing_truncates_low_byte_on_cuda() {
    let backend = live_backend();
    let gpu = run(&backend, DataType::U8, DataType::U32);
    let expected: Vec<u32> = inputs().iter().map(|&v| u32::from(v as u8)).collect();
    assert_eq!(
        expected,
        vec![44, 0x45, 200, 0xFF, 0, 0xFF, 0, 127, 128, 255],
        "reference u32->u8 truncation drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->u8 narrowing diverged from `as u8` (cvt.u32.u8 skipped?).\n  inputs:   {:?}\n  expected: {:?}\n  gpu:      {:?}",
        inputs(), expected, gpu
    );
}

#[test]
fn u32_to_u16_narrowing_truncates_low_half_on_cuda() {
    let backend = live_backend();
    let gpu = run(&backend, DataType::U16, DataType::U32);
    let expected: Vec<u32> = inputs().iter().map(|&v| u32::from(v as u16)).collect();
    assert_eq!(
        expected,
        vec![300, 0x2345, 200, 0xFFFF, 0x8000, 0xFFFF, 0, 127, 128, 255],
        "reference u32->u16 truncation drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->u16 narrowing diverged from `as u16`.\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn u32_to_i8_narrowing_sign_extends_on_cuda() {
    let backend = live_backend();
    let gpu: Vec<i32> = run(&backend, DataType::I8, DataType::I32)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let expected: Vec<i32> = inputs().iter().map(|&v| i32::from(v as u8 as i8)).collect();
    assert_eq!(
        expected,
        vec![44, 69, -56, -1, 0, -1, 0, 127, -128, -1],
        "reference u32->i8 sign-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->i8 narrowing diverged from `as i8` (cvt.s32.s8 sign-extend?).\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn u32_to_i16_narrowing_sign_extends_on_cuda() {
    let backend = live_backend();
    let gpu: Vec<i32> = run(&backend, DataType::I16, DataType::I32)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let expected: Vec<i32> = inputs()
        .iter()
        .map(|&v| i32::from(v as u16 as i16))
        .collect();
    assert_eq!(
        expected,
        vec![300, 0x2345, 200, -1, -32768, -1, 0, 127, 128, 255],
        "reference u32->i16 sign-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "CUDA u32->i16 narrowing diverged from `as i16`.\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

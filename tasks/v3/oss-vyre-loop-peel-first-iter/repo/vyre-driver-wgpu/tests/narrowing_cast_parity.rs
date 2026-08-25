//! Integer-narrowing cast parity against the reference oracle / Rust `as`.
//!
//! A narrowing cast (u32 -> u8/u16/i8/i16) is validate-LEGAL (V035 only WARNS
//! "narrowing cast may truncate high bits"), so it reaches the GPU. WGSL has no
//! 8/16-bit scalar register, so `scalar_cast_target` backs U8/U16 with a u32 and
//! I8/I16 with an i32; the bare `As` that produces that register is a no-op for a
//! same-width source. Before the narrowing fix, `300u32 as u8` therefore STAYED
//! 300 on the GPU instead of truncating to 44 — a silent divergence from Rust
//! `as`, the V035 contract, and the reference oracle (the div-by-zero /
//! shift-mask silent-divergence class, Law 10).
//!
//! This test dispatches the narrowing on the live GPU and asserts the truncated
//! value byte-for-byte. To isolate the CAST's narrowing from the byte-element
//! STORE (which masks to a byte regardless), it widens the narrowed value back
//! out — `cast(WIDE, cast(NARROW, x))` — and stores into a 32-bit buffer, so the
//! word read back reflects exactly what the narrowing cast produced.

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// Probe inputs (as u32 bit patterns) that exercise truncation and the signed
/// boundary: 300 (low byte 44), 0x12345 (low half 0x2345), 200 (i8 -56), 0xFFFF
/// (i16 -1 / u16 max), 0x8000 (i16 MIN), 0xFFFFFFFF (all ones), 0, 127, 128, 255.
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

fn run(backend: &WgpuBackend, narrow: DataType, wide: DataType) -> Vec<u32> {
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
        .expect("Fix: WGPU must dispatch the narrowing-cast parity contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn u32_to_u8_narrowing_truncates_low_byte_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: narrowing-cast parity requires a live GPU backend.");
    let gpu = run(&backend, DataType::U8, DataType::U32);
    let expected: Vec<u32> = inputs().iter().map(|&v| u32::from(v as u8)).collect();
    // Pin the contract literally: 300->44, 0x12345->0x45, 200->200, 0xFFFF->0xFF,
    // 0x8000->0, 0xFFFFFFFF->0xFF, 0->0, 127->127, 128->128, 255->255.
    assert_eq!(
        expected,
        vec![44, 0x45, 200, 0xFF, 0, 0xFF, 0, 127, 128, 255],
        "reference u32->u8 truncation drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32->u8 narrowing diverged from `as u8`.\n  inputs:   {:?}\n  expected: {:?}\n  gpu:      {:?}",
        inputs(), expected, gpu
    );
}

#[test]
fn u32_to_u16_narrowing_truncates_low_half_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: narrowing-cast parity requires a live GPU backend.");
    let gpu = run(&backend, DataType::U16, DataType::U32);
    let expected: Vec<u32> = inputs().iter().map(|&v| u32::from(v as u16)).collect();
    assert_eq!(
        expected,
        vec![300, 0x2345, 200, 0xFFFF, 0x8000, 0xFFFF, 0, 127, 128, 255],
        "reference u32->u16 truncation drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32->u16 narrowing diverged from `as u16`.\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn u32_to_i8_narrowing_sign_extends_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: narrowing-cast parity requires a live GPU backend.");
    let gpu: Vec<i32> = run(&backend, DataType::I8, DataType::I32)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let expected: Vec<i32> = inputs().iter().map(|&v| i32::from(v as u8 as i8)).collect();
    // 300->44, 0x12345->0x45(69), 200->-56, 0xFFFF->-1, 0x8000->0, 0xFFFFFFFF->-1,
    // 0->0, 127->127, 128->-128, 255->-1.
    assert_eq!(
        expected,
        vec![44, 69, -56, -1, 0, -1, 0, 127, -128, -1],
        "reference u32->i8 sign-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32->i8 narrowing diverged from `as i8`.\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn u32_to_i16_narrowing_sign_extends_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: narrowing-cast parity requires a live GPU backend.");
    let gpu: Vec<i32> = run(&backend, DataType::I16, DataType::I32)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let expected: Vec<i32> = inputs()
        .iter()
        .map(|&v| i32::from(v as u16 as i16))
        .collect();
    // 0xFFFF->-1, 0x8000->-32768, others positive low-half values.
    assert_eq!(
        expected,
        vec![300, 0x2345, 200, -1, -32768, -1, 0, 127, 128, 255],
        "reference u32->i16 sign-extension drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32->i16 narrowing diverged from `as i16`.\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

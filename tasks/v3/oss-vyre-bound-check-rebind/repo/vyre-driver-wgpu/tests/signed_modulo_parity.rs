//! Signed integer modulo parity against Rust `%` on the live GPU.
//!
//! naga's `BinaryOperator::Modulo` lowers to an UNSIGNED remainder on the SPIR-V
//! backend even for signed operands — a vendored-naga bug: on the 5090,
//! `rem(i32, i32)` of (-7, 3) returned 0 (== unsigned `0xFFFF_FFF9 % 3`) instead
//! of the signed -1, while `div(i32, i32)` of (-7, 3) correctly returned -2. The
//! emitter now synthesizes signed remainder from the truncating-division identity
//! `a - (a / b) * b` (naga's `Divide` IS signedness-correct). This test dispatches
//! signed modulo on real hardware and asserts byte-for-byte agreement with Rust
//! `%`, the regression guard for that fix.
//!
//! (The inventory's 2026-06-18 "signed mod emits SRem, correct" conclusion was a
//! source-read that was never GPU-verified; this test is the empirical truth.)

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

fn pairs() -> Vec<(i32, i32)> {
    vec![(-7, 3), (7, 3), (-8, 3), (100, 7), (-100, 7), (-1, 2), (5, -3), (-2147483648, 3)]
}

/// `out[i] = a[i] % b[i]` — all I32 buffers; the rem is i32 % i32 (signed).
fn rem_program(n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::rem(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::I32).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::I32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::I32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

/// `out[i] = a[i] / b[i]` — all I32 (the signed-Div twin, must stay correct).
fn div_program(n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::div(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::I32).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::I32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::I32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn run(backend: &WgpuBackend, program: &Program, ps: &[(i32, i32)]) -> Vec<i32> {
    let a = u32_bytes(&ps.iter().map(|&(a, _)| a as u32).collect::<Vec<_>>());
    let b = u32_bytes(&ps.iter().map(|&(_, b)| b as u32).collect::<Vec<_>>());
    let out_init = u32_bytes(&vec![0u32; ps.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the signed modulo contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn signed_modulo_matches_rust_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: signed modulo parity requires a live GPU backend.");
    let ps = pairs();
    let gpu = run(&backend, &rem_program(ps.len() as u32), &ps);
    // i32::MIN % 3 in Rust truncated remainder = -2; the divide a/b for that pair
    // is well-defined (only i32::MIN / -1 overflows, which we don't test).
    let expected: Vec<i32> = ps.iter().map(|&(a, b)| a % b).collect();
    assert_eq!(
        expected,
        vec![-1, 1, -2, 2, -2, -1, 2, -2],
        "Rust signed-remainder reference drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU signed modulo diverged from Rust `%` (the naga unsigned-Modulo bug regressed).\n  \
         pairs: {ps:?}\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn signed_division_matches_rust_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: signed division parity requires a live GPU backend.");
    let ps = pairs();
    let gpu = run(&backend, &div_program(ps.len() as u32), &ps);
    let expected: Vec<i32> = ps.iter().map(|&(a, b)| a / b).collect();
    assert_eq!(
        expected,
        vec![-2, 2, -2, 14, -14, 0, -1, -715827882],
        "Rust signed-division reference drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU signed division diverged from Rust `/`.\n  pairs: {ps:?}\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

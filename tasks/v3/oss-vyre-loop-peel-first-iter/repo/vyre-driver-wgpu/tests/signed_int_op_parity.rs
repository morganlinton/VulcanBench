//! Signed integer operation parity against Rust on the live GPU.
//!
//! The naga signed-`Modulo`-emits-unsigned bug (fixed in the emitter) was a
//! silent miscompile the existing GPU sweeps missed because they under-cover
//! NEGATIVE i32 operands. This locks the rest of the signedness-sensitive integer
//! ops — `min`/`max` (signed SMin/SMax) and the ordered comparisons (signed
//! SLessThan etc.) — against the same class of regression. All are dispatched on
//! the 5090 with negative operands and asserted byte-for-byte against Rust.
//!
//! (Division and modulo have their own guard in `signed_modulo_parity`.)

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// Operand pairs spanning sign combinations, including the extremes.
fn pairs() -> Vec<(i32, i32)> {
    vec![
        (-7, 3),
        (3, -7),
        (-8, -3),
        (-100, 7),
        (5, -200),
        (-1, -1),
        (i32::MIN, i32::MAX),
        (i32::MAX, i32::MIN),
        (0, -1),
    ]
}

fn program(elem: DataType, n: u32, f: fn(Expr, Expr) -> Expr) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            f(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, elem).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::I32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::I32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn dispatch(backend: &WgpuBackend, program: &Program, ps: &[(i32, i32)]) -> Vec<u32> {
    let a = u32_bytes(&ps.iter().map(|&(a, _)| a as u32).collect::<Vec<_>>());
    let b = u32_bytes(&ps.iter().map(|&(_, b)| b as u32).collect::<Vec<_>>());
    let out_init = u32_bytes(&vec![0u32; ps.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the signed int-op contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn signed_min_max_match_rust_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: signed int-op parity requires a live GPU backend.");
    let ps = pairs();
    let n = ps.len() as u32;

    let gpu_min: Vec<i32> = dispatch(&backend, &program(DataType::I32, n, Expr::min), &ps)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let exp_min: Vec<i32> = ps.iter().map(|&(a, b)| a.min(b)).collect();
    assert_eq!(
        gpu_min, exp_min,
        "GPU signed min diverged from Rust (would be unsigned if naga SMin regressed).\n  \
         pairs: {ps:?}\n  expected: {exp_min:?}\n  gpu: {gpu_min:?}"
    );

    let gpu_max: Vec<i32> = dispatch(&backend, &program(DataType::I32, n, Expr::max), &ps)
        .into_iter()
        .map(|w| w as i32)
        .collect();
    let exp_max: Vec<i32> = ps.iter().map(|&(a, b)| a.max(b)).collect();
    assert_eq!(
        gpu_max, exp_max,
        "GPU signed max diverged from Rust.\n  pairs: {ps:?}\n  expected: {exp_max:?}\n  gpu: {gpu_max:?}"
    );
}

#[test]
fn signed_comparisons_match_rust_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: signed comparison parity requires a live GPU backend.");
    let ps = pairs();
    let n = ps.len() as u32;

    // Each comparison produces a Bool stored straight into a U32 buffer (0/1).
    let cases: [(&str, fn(Expr, Expr) -> Expr, fn(i32, i32) -> bool); 6] = [
        ("lt", Expr::lt, |a, b| a < b),
        ("gt", Expr::gt, |a, b| a > b),
        ("le", Expr::le, |a, b| a <= b),
        ("ge", Expr::ge, |a, b| a >= b),
        ("eq", Expr::eq, |a, b| a == b),
        ("ne", Expr::ne, |a, b| a != b),
    ];
    for (name, build, rust) in cases {
        let gpu = dispatch(&backend, &program(DataType::U32, n, build), &ps);
        let expected: Vec<u32> = ps.iter().map(|&(a, b)| u32::from(rust(a, b))).collect();
        assert_eq!(
            gpu, expected,
            "GPU signed `{name}` diverged from Rust (would differ if naga used an unsigned compare).\n  \
             pairs: {ps:?}\n  expected: {expected:?}\n  gpu: {gpu:?}"
        );
    }
}

//! Div/mod-by-zero and oversized-shift parity against the reference oracle on
//! the live GPU — the "undefined on hardware, TOTAL on the oracle" class (Law 10).
//!
//! Three operations have hardware-undefined behavior that the vyre-reference
//! oracle nonetheless defines with a single total contract, so the wgpu backend
//! must force that contract or silently disagree with its own oracle:
//!
//!   * `u32 x / 0`  -> `u32::MAX`   (oracle `div_u32`; naga would yield `x`,
//!                                   PTX leaves it to unspecified hardware)
//!   * `u32 x % 0`  -> `0`          (oracle `rem_u32`)
//!   * `u32 x << s` / `x >> s` with `s >= 32` -> shift amount taken `& 31`
//!                                   (oracle `shift_u32`; SPIR-V/WGSL mask the
//!                                   amount to the bit width, but that is never
//!                                   verified against the oracle on real silicon)
//!
//! op_dispatch forces the div/mod sentinels with a `Select(divisor == 0, ...)`.
//! That Select-forced value is a COMPUTED lowering exactly like the naga
//! signed-`Modulo` bug — a source read ("we emit a Select to u32::MAX") is NOT
//! proof the 5090 returns `u32::MAX`. These tests dispatch all three on real
//! hardware and assert byte-for-byte against the oracle contract.
//!
//! (Signed `i32 / 0` and `i32::MIN / -1` are rejected upstream as undefined —
//! `div_i32`/`rem_i32` return an error — so they are not emittable and not
//! tested here; only the unsigned, total cases reach the GPU.)

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// `out[i] = op(a[i], b[i])` over all-U32 buffers, `op` built from two loads.
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
        .expect("Fix: WGPU must dispatch the div-zero / shift-mask parity contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Divisor cases including the zero-divisor sentinels and normal control values.
fn div_cases() -> Vec<(u32, u32)> {
    vec![
        (10, 0),
        (0, 0),
        (u32::MAX, 0),
        (123, 0),
        (100, 7),
        (0, 5),
        (u32::MAX, 1),
        (4096, 4096),
    ]
}

#[test]
fn u32_div_by_zero_yields_max_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: div-zero parity requires a live GPU backend.");
    let ps = div_cases();
    let gpu = dispatch(&backend, &program(ps.len() as u32, Expr::div), &ps);
    let expected: Vec<u32> = ps
        .iter()
        .map(|&(a, b)| if b == 0 { u32::MAX } else { a / b })
        .collect();
    // Pin the contract literally: every zero divisor -> u32::MAX, never `a`.
    assert_eq!(
        expected,
        vec![u32::MAX, u32::MAX, u32::MAX, u32::MAX, 14, 0, u32::MAX, 1],
        "reference u32 div-by-zero contract drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32 `/ 0` diverged from the oracle (`x / 0 == u32::MAX`). A bare naga \
         Divide would yield `x` here, silently disagreeing with the oracle.\n  \
         cases: {ps:?}\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

#[test]
fn u32_mod_by_zero_yields_zero_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: mod-zero parity requires a live GPU backend.");
    let ps = div_cases();
    let gpu = dispatch(&backend, &program(ps.len() as u32, Expr::rem), &ps);
    let expected: Vec<u32> = ps
        .iter()
        .map(|&(a, b)| if b == 0 { 0 } else { a % b })
        .collect();
    assert_eq!(
        expected,
        vec![0, 0, 0, 0, 2, 0, 0, 0],
        "reference u32 mod-by-zero contract drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32 `% 0` diverged from the oracle (`x % 0 == 0`).\n  cases: {ps:?}\n  \
         expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

/// (value, shift-amount) — the amounts >= 32 exercise the `& 31` masking that a
/// non-masking lowering would get wrong (e.g. `1 << 32` would be 0, not 1).
fn shift_cases() -> Vec<(u32, u32)> {
    vec![
        (1, 0),
        (1, 31),
        (1, 32),
        (1, 33),
        (0xFF, 36),
        (0xFFFF_FFFF, 32),
        (0x1, 63),
        (0xDEAD_BEEF, 4),
    ]
}

#[test]
fn u32_oversized_shift_left_masks_amount_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: shift-mask parity requires a live GPU backend.");
    let ps = shift_cases();
    let gpu = dispatch(&backend, &program(ps.len() as u32, Expr::shl), &ps);
    // Oracle `shift_u32`: left << (right & 31). wrapping_shl masks identically.
    let expected: Vec<u32> = ps.iter().map(|&(v, s)| v.wrapping_shl(s)).collect();
    // The load-bearing oversized cases: 1<<32 -> 1 (NOT 0), 0xFFFFFFFF<<32 ->
    // 0xFFFFFFFF (NOT 0), 1<<33 -> 2, 1<<63 -> 0x80000000.
    assert_eq!(
        expected,
        vec![1, 0x8000_0000, 1, 2, 0xFF0, 0xFFFF_FFFF, 0x8000_0000, 0xEADB_EEF0],
        "reference u32 oversized shift-left contract drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32 `<<` with amount >= 32 diverged from the oracle (`<< (s & 31)`). A \
         non-masking shift would zero `1 << 32`.\n  cases: {ps:?}\n  expected: {expected:?}\n  \
         gpu: {gpu:?}"
    );
}

#[test]
fn u32_oversized_shift_right_masks_amount_on_gpu() {
    let backend =
        WgpuBackend::acquire().expect("Fix: shift-mask parity requires a live GPU backend.");
    let ps = shift_cases();
    let gpu = dispatch(&backend, &program(ps.len() as u32, Expr::shr), &ps);
    let expected: Vec<u32> = ps.iter().map(|&(v, s)| v.wrapping_shr(s)).collect();
    // 1>>32 -> 1 (32&31==0), 0xFFFFFFFF>>32 -> 0xFFFFFFFF, 0xFF>>36 -> 0xFF>>4 == 0xF.
    assert_eq!(
        expected,
        vec![1, 0, 1, 0, 0xF, 0xFFFF_FFFF, 0, 0x0DEA_DBEE],
        "reference u32 oversized shift-right contract drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU u32 `>>` with amount >= 32 diverged from the oracle (`>> (s & 31)`).\n  \
         cases: {ps:?}\n  expected: {expected:?}\n  gpu: {gpu:?}"
    );
}

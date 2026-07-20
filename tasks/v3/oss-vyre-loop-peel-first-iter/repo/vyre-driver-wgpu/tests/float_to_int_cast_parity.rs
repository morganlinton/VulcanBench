//! Float→integer cast parity against the reference oracle (Rust saturating `as`).
//!
//! The reference oracle (`vyre-reference/src/execution/expr_cast.rs`) lowers
//! `Cast{F32→U32/I32}` with Rust's `as`, which is SATURATING since Rust 1.45:
//!   * in-range          → truncate toward zero
//!   * +∞ / overflow     → target MAX
//!   * −∞ / underflow     → target MIN (0 for unsigned)
//!   * **NaN             → 0**
//!
//! vyre-emit-naga emits a bare `As{Uint/Sint}`; naga's SPIR-V backend wraps it
//! in `FClamp(x, min, max)` then `ConvertFToU/S`, so ±∞ and out-of-range
//! saturate correctly. BUT `FClamp(NaN, …)` reduces to FMin/FMax of a NaN,
//! whose result is UNDEFINED per the SPIR-V GLSL.std.450 spec (it may return
//! min, max, or NaN by hardware). The output is an integer, so the differential
//! harness's ULP tolerance cannot mask a divergence. A legal kernel that casts a
//! possibly-NaN float (e.g. `(0.0/0.0) as u32`) can therefore silently disagree
//! with the oracle on the GPU — the div-by-zero / shift-mask silent-divergence
//! class (Law 10).
//!
//! This test dispatches the cast on the live GPU and asserts byte-for-byte
//! equality with the saturating reference, NaN included.

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// The probe inputs: normal, out-of-range, and the IEEE specials.
fn cast_inputs() -> Vec<f32> {
    vec![
        42.9,
        -5.0,
        3.0e20,
        -3.0e20,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ]
}

/// Build a kernel that loads each f32, casts it to `target`, and stores the
/// result word-for-word into `out`.
fn cast_program(target: DataType, n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::cast(target.clone(), Expr::load("input", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, target).with_count(n),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn run_cast(backend: &WgpuBackend, target: DataType) -> Vec<u32> {
    run_cast_inputs(backend, target, &cast_inputs())
}

fn run_cast_inputs(backend: &WgpuBackend, target: DataType, inputs: &[f32]) -> Vec<u32> {
    let n = inputs.len() as u32;
    let program = cast_program(target, n);
    // Pack the raw f32 bit patterns into the input buffer.
    let input_bytes = u32_bytes(&inputs.iter().map(|f| f.to_bits()).collect::<Vec<_>>());
    let out_init = u32_bytes(&vec![0u32; inputs.len()]);
    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[out_init.as_slice(), input_bytes.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the float→int cast parity contract.");
    // One output buffer of `n` u32 words.
    let bytes = &outputs[0];
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn f32_to_u32_cast_saturates_like_reference_including_nan() {
    let backend = WgpuBackend::acquire()
        .expect("Fix: float→int cast parity requires a live GPU backend.");
    let gpu = run_cast(&backend, DataType::U32);

    // Reference oracle = Rust saturating `as` (f32 widened to f64, then `as u32`).
    let expected: Vec<u32> = cast_inputs()
        .iter()
        .map(|&f| f64::from(f) as u32)
        .collect();

    // Pin the contract literally so a silent reference change can't hide a drift:
    // [42, 0(neg→0), MAX(overflow), 0(neg overflow→0), MAX(+∞), 0(−∞→0), 0(NaN)].
    assert_eq!(
        expected,
        vec![42, 0, u32::MAX, 0, u32::MAX, 0, 0],
        "reference saturating semantics drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU f32→u32 cast diverged from the saturating reference.\n  inputs:   {:?}\n  expected: {:?}\n  gpu:      {:?}\n\
         The NaN slot (last) is the likely diverger: FClamp(NaN) is SPIR-V-undefined.",
        cast_inputs(),
        expected,
        gpu
    );
}

#[test]
fn computed_f32_overflow_casts_saturate_like_reference() {
    // The saturating guard must fire for a COMPUTED float source (an arithmetic
    // result), not only a buffer load — the emitter detects the float source via
    // both the bound type handle AND the scalar-kind resolver, so a computed
    // float can't silently skip the guard back onto the diverging bare `As`.
    // fma(1e20, 1e20, 0) overflows: +inf in f32 on the GPU, 1e40 in the f64
    // reference; both saturate through the cast to the integer max.
    let backend = WgpuBackend::acquire()
        .expect("Fix: float→int cast parity requires a live GPU backend.");

    let program = |target: DataType| {
        Program::wrapped(
            vec![
                BufferDecl::storage("out", 0, BufferAccess::ReadWrite, target.clone()).with_count(1),
                BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::F32).with_count(1),
                BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::F32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::cast(
                    target,
                    Expr::fma(
                        Expr::load("a", Expr::u32(0)),
                        Expr::load("b", Expr::u32(0)),
                        Expr::f32(0.0),
                    ),
                ),
            )],
        )
    };
    let a = u32_bytes(&[1.0e20_f32.to_bits()]);
    let b = u32_bytes(&[1.0e20_f32.to_bits()]);

    let u_out = backend
        .dispatch_borrowed(
            &program(DataType::U32),
            &[u32_bytes(&[0]).as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("dispatch computed f32->u32");
    let u = u32::from_le_bytes([u_out[0][0], u_out[0][1], u_out[0][2], u_out[0][3]]);
    assert_eq!(
        u,
        u32::MAX,
        "computed fma overflow → u32 must saturate to u32::MAX (got {u})"
    );

    let i_out = backend
        .dispatch_borrowed(
            &program(DataType::I32),
            &[u32_bytes(&[0]).as_slice(), a.as_slice(), b.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("dispatch computed f32->i32");
    let i = u32::from_le_bytes([i_out[0][0], i_out[0][1], i_out[0][2], i_out[0][3]]) as i32;
    assert_eq!(
        i,
        i32::MAX,
        "computed fma overflow → i32 must saturate to i32::MAX (got {i})"
    );
}

#[test]
fn f32_to_i32_cast_saturates_like_reference_including_nan() {
    let backend = WgpuBackend::acquire()
        .expect("Fix: float→int cast parity requires a live GPU backend.");
    let gpu_bits = run_cast(&backend, DataType::I32);
    let gpu: Vec<i32> = gpu_bits.iter().map(|&w| w as i32).collect();

    let expected: Vec<i32> = cast_inputs()
        .iter()
        .map(|&f| f64::from(f) as i32)
        .collect();

    // [42, -5, MAX(overflow), MIN(neg overflow), MAX(+∞), MIN(−∞), 0(NaN)].
    assert_eq!(
        expected,
        vec![42, -5, i32::MAX, i32::MIN, i32::MAX, i32::MIN, 0],
        "reference saturating semantics drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU f32→i32 cast diverged from the saturating reference.\n  inputs:   {:?}\n  expected: {:?}\n  gpu:      {:?}",
        cast_inputs(),
        expected,
        gpu
    );
}

#[test]
fn f32_to_int_cast_threshold_boundaries_match_reference() {
    // The fix's overflow threshold is 2^31 (i32) / 2^32 (u32). Exercise the exact
    // boundary: the largest f32 strictly inside the integer range (must convert
    // exactly, NOT saturate) vs the first f32 at/above the threshold (must
    // saturate to INT_MAX). There is no representable f32 between these two, so
    // the threshold must land precisely or an in-range value wrongly saturates
    // (or an overflow wrongly converts). Negative boundaries confirm the FClamp
    // low bound (i32::MIN is exactly f32-representable) still matches.
    let backend = WgpuBackend::acquire()
        .expect("Fix: float→int cast parity requires a live GPU backend.");

    // i32 boundaries.
    //   2147483520 = 0x4EFFFFFF f32 = largest f32 <= i32::MAX (in range, exact).
    //   2147483648 = 2^31       f32 = first f32 above i32::MAX (overflow).
    //  -2147483648 = -2^31      f32 = i32::MIN exactly (in range).
    //  -2147483904               = next f32 below -2^31 (underflow -> i32::MIN).
    let i_inputs = [
        2_147_483_520.0_f32,
        2_147_483_648.0_f32,
        -2_147_483_648.0_f32,
        -2_147_483_904.0_f32,
    ];
    let gpu_i: Vec<i32> = run_cast_inputs(&backend, DataType::I32, &i_inputs)
        .iter()
        .map(|&w| w as i32)
        .collect();
    let ref_i: Vec<i32> = i_inputs.iter().map(|&f| f64::from(f) as i32).collect();
    assert_eq!(
        ref_i,
        vec![2_147_483_520, i32::MAX, i32::MIN, i32::MIN],
        "i32 reference boundary values drifted"
    );
    assert_eq!(
        gpu_i, ref_i,
        "GPU f32→i32 boundary cast diverged.\n  inputs:   {i_inputs:?}\n  expected: {ref_i:?}\n  gpu:      {gpu_i:?}"
    );

    // u32 boundaries.
    //   4294967040 = largest f32 <= u32::MAX (in range, exact).
    //   4294967296 = 2^32 = first f32 above u32::MAX (overflow).
    let u_inputs = [4_294_967_040.0_f32, 4_294_967_296.0_f32, 0.0_f32];
    let gpu_u = run_cast_inputs(&backend, DataType::U32, &u_inputs);
    let ref_u: Vec<u32> = u_inputs.iter().map(|&f| f64::from(f) as u32).collect();
    assert_eq!(
        ref_u,
        vec![4_294_967_040, u32::MAX, 0],
        "u32 reference boundary values drifted"
    );
    assert_eq!(
        gpu_u, ref_u,
        "GPU f32→u32 boundary cast diverged.\n  inputs:   {u_inputs:?}\n  expected: {ref_u:?}\n  gpu:      {gpu_u:?}"
    );
}

//! Overflow-guard regression tests for Cat-A composition builders.
//!
//! When a caller passes shape parameters whose product overflows u32,
//! the builders MUST panic with an actionable message rather than
//! silently produce an under-sized buffer + OOB memory access at
//! runtime. The guards use `checked_mul`; these tests prove every
//! exit path fires at the correct boundary.

#![cfg(all(feature = "math-linalg", feature = "nn-attention"))]

use vyre_libs::math::{matmul, matmul_tiled, Matmul, MatmulTiled};
use vyre_libs::nn::{attention, Attention};
use vyre_libs::tensor_ref::TensorRef;

/// Near-overflow bound: any `a*b > u32::MAX` must panic.
const PIVOT: u32 = 1u32 << 16; // 65_536; 65_536 * 65_536 = u32::MAX + 1

#[test]
fn matmul_m_times_k_overflow_returns_error() {
    let error = Matmul::new(
        TensorRef::u32_2d("a", PIVOT, PIVOT),
        TensorRef::u32_2d("b", PIVOT, 1),
        TensorRef::u32_2d("out", PIVOT, 1),
    )
    .build()
    .expect_err("Fix: m*k overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn matmul_k_times_n_overflow_returns_error() {
    // m=1 keeps m*k safe; k*n is the overflow.
    let error = Matmul::new(
        TensorRef::u32_2d("a", 1, PIVOT),
        TensorRef::u32_2d("b", PIVOT, PIVOT),
        TensorRef::u32_2d("out", 1, PIVOT),
    )
    .build()
    .expect_err("Fix: k*n overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn matmul_m_times_n_overflow_returns_error() {
    // m*k safe (m=1), k*n safe (k=1), m*n overflows.
    let error = Matmul::new(
        TensorRef::u32_2d("a", PIVOT, 1),
        TensorRef::u32_2d("b", 1, PIVOT),
        TensorRef::u32_2d("out", PIVOT, PIVOT),
    )
    .build()
    .expect_err("Fix: m*n overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn matmul_tiled_m_times_k_overflow_returns_error() {
    let error = MatmulTiled::new(
        TensorRef::u32_2d("a", PIVOT, PIVOT),
        TensorRef::u32_2d("b", PIVOT, 1),
        TensorRef::u32_2d("out", PIVOT, 1),
        16,
    )
    .build()
    .expect_err("Fix: tiled m*k overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn matmul_tiled_k_times_n_overflow_returns_error() {
    let error = MatmulTiled::new(
        TensorRef::u32_2d("a", 1, PIVOT),
        TensorRef::u32_2d("b", PIVOT, PIVOT),
        TensorRef::u32_2d("out", 1, PIVOT),
        16,
    )
    .build()
    .expect_err("Fix: tiled k*n overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn matmul_tiled_m_times_n_overflow_returns_error() {
    let error = MatmulTiled::new(
        TensorRef::u32_2d("a", PIVOT, 1),
        TensorRef::u32_2d("b", 1, PIVOT),
        TensorRef::u32_2d("out", PIVOT, PIVOT),
        16,
    )
    .build()
    .expect_err("Fix: tiled m*n overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn attention_s_times_d_overflow_returns_error() {
    let error = Attention::new(
        TensorRef::f32_2d("q", PIVOT, PIVOT),
        TensorRef::f32_2d("k", PIVOT, PIVOT),
        TensorRef::f32_2d("v", PIVOT, PIVOT),
        TensorRef::f32_2d("out", PIVOT, PIVOT),
    )
    .build()
    .expect_err("Fix: attention s*d overflow must return an error contract");
    assert!(error.to_string().contains("element-count overflows u32"));
}

#[test]
fn reasonable_shapes_build_without_panic() {
    // Shapes under the pivot must continue to work end-to-end.
    let _ = matmul("a", "b", "out", 64, 64, 64);
    let _ = matmul_tiled("a", "b", "out", 64, 64, 64, 16);
    let _ = attention("q", "k", "v", "out", 128, 64);
}

#[test]
fn just_under_pivot_succeeds_for_one_operand() {
    // 65_535 * 65_535 = 4_294_836_225 < u32::MAX. Still safe.
    let _ = matmul("a", "b", "out", 65_535, 1, 65_535);
}

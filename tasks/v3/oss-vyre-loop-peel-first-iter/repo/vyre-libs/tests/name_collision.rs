//! P2.3: Every typed Cat-A builder must reject buffer-name collisions
//! at `build()` time. A collision means two TensorRefs share a buffer
//! name  -  execution writes and reads through the same storage, which
//! corrupts the output silently.

#![cfg(all(feature = "math-linalg", feature = "nn-attention", feature = "nn-norm",))]

use vyre_libs::math::{Matmul, MatmulTiled};
use vyre_libs::nn::{Attention, LayerNorm, Softmax};
use vyre_libs::tensor_ref::{TensorRef, TensorRefError};

#[test]
fn softmax_rejects_input_equals_output_name() {
    let err = Softmax::new(
        TensorRef::f32_1d("shared", 4),
        TensorRef::f32_1d("shared", 4),
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn layer_norm_rejects_input_equals_output_name() {
    let err = LayerNorm::new(
        TensorRef::f32_1d("shared", 4),
        TensorRef::f32_1d("shared", 4),
        1e-5,
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn attention_rejects_q_equals_k_name() {
    let err = Attention::new(
        TensorRef::f32_2d("shared", 4, 8),
        TensorRef::f32_2d("shared", 4, 8),
        TensorRef::f32_2d("v", 4, 8),
        TensorRef::f32_2d("out", 4, 8),
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn attention_rejects_v_equals_out_name() {
    let err = Attention::new(
        TensorRef::f32_2d("q", 4, 8),
        TensorRef::f32_2d("k", 4, 8),
        TensorRef::f32_2d("shared", 4, 8),
        TensorRef::f32_2d("shared", 4, 8),
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn matmul_rejects_a_equals_b_name() {
    let err = Matmul::new(
        TensorRef::u32_2d("shared", 4, 8),
        TensorRef::u32_2d("shared", 8, 4),
        TensorRef::u32_2d("out", 4, 4),
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn matmul_tiled_rejects_a_equals_out_name() {
    let err = MatmulTiled::new(
        TensorRef::u32_2d("shared", 4, 8),
        TensorRef::u32_2d("b", 8, 4),
        TensorRef::u32_2d("shared", 4, 4),
        4,
    )
    .build()
    .unwrap_err();
    assert!(matches!(err, TensorRefError::NameCollision { .. }));
}

#[test]
fn distinct_names_succeed_across_all_builders() {
    // Positive path: every builder constructs cleanly when names differ.
    Softmax::new(TensorRef::f32_1d("a", 4), TensorRef::f32_1d("b", 4))
        .build()
        .unwrap();
    LayerNorm::new(TensorRef::f32_1d("a", 4), TensorRef::f32_1d("b", 4), 1e-5)
        .build()
        .unwrap();
    Attention::new(
        TensorRef::f32_2d("q", 4, 8),
        TensorRef::f32_2d("k", 4, 8),
        TensorRef::f32_2d("v", 4, 8),
        TensorRef::f32_2d("out", 4, 8),
    )
    .build()
    .unwrap();
    Matmul::new(
        TensorRef::u32_2d("a", 4, 8),
        TensorRef::u32_2d("b", 8, 4),
        TensorRef::u32_2d("out", 4, 4),
    )
    .build()
    .unwrap();
    MatmulTiled::new(
        TensorRef::u32_2d("a", 4, 8),
        TensorRef::u32_2d("b", 8, 4),
        TensorRef::u32_2d("out", 4, 4),
        4,
    )
    .build()
    .unwrap();
}

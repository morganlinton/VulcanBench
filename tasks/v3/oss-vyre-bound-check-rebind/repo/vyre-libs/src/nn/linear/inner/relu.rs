//! Fused `linear_relu` constructor.

use vyre::ir::{DataType, Program};

use super::fused_activation::linear_fused_activation;
use crate::nn::activation::relu::relu_f32_expr;

const OP_ID: &str = "vyre-libs::nn::linear_relu";

/// Build a Program that computes `out[i] = max(0, sum_k x[k] * w[k, i] + b[i])`.
///
/// Fused variant of `linear` followed by ReLU.
///
/// # Errors
/// Returns `Err` when `in_dim == 0`.
pub fn linear_relu(
    x: &str,
    w: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
) -> Result<Program, String> {
    linear_fused_activation(
        "linear_relu",
        OP_ID,
        x,
        w,
        b,
        out,
        in_dim,
        out_dim,
        relu_f32_expr,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            linear_relu("x", "w", "b", "out", 4, 4).unwrap_or_else(|error| {
                crate::builder::invalid_output_program(
                    OP_ID,
                    "out",
                    DataType::F32,
                    error,
                )
            })
        },
        test_inputs: Some(|| {
            let f32_bytes = vyre_primitives::wire::pack_f32_slice;
            let x = f32_bytes(&(0..4).map(|i| i as f32).collect::<Vec<_>>());
            let w = f32_bytes(&(0..16).map(|i| i as f32).collect::<Vec<_>>());
            let bias = f32_bytes(&[0.0, 0.0, 0.0, 0.0]);
            vec![vec![x, w, bias]]
        }),
        expected_output: Some(|| {
            let f32_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![f32_bytes(&[56.0, 62.0, 68.0, 74.0])]]
        }),
        category: Some("nn"),
    }
}

//! Layerwise LN scale: `y = layer_norm(x) * scale`.
//!
//! Category A  -  element-wise mul by per-dim learnable scale.

use crate::math::elementwise::{f32_elementwise_mul, F32MulRhs};
use vyre::ir::Program;

const OP_ID: &str = "vyre-libs::nn::layerwise_ln_scale";

/// Build a Program: `output[i] = input[i] * scale[i]` (F32).
#[must_use]
pub fn layerwise_ln_scale(input: &str, scale: &str, output: &str, n: u32) -> Program {
    f32_elementwise_mul(OP_ID, input, F32MulRhs::Buffer(scale), output, n)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || layerwise_ln_scale("input", "scale", "output", 4),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0]),  // input (post-LN)
                to_f32(&[0.5, 2.0, 1.0, 0.1]),  // scale
            ]]
        }),
        expected_output: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![to_f32(&[0.5, 4.0, 3.0, 0.4])]]
        }),
        category: Some("nn"),
    }
}

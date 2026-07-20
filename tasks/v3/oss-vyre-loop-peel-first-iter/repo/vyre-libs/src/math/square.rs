//! Element-wise square: `y = x * x`.
//!
//! Category-A composition.

use crate::math::elementwise::{f32_elementwise_mul, F32MulRhs};
use vyre::ir::Program;

/// Build a Program that computes `output[i] = input[i] * input[i]`.
#[must_use]
pub fn square(input: &str, output: &str, n: u32) -> Program {
    f32_elementwise_mul(
        "vyre-libs::math::square",
        input,
        F32MulRhs::SameInput,
        output,
        n,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::square",
        build: || square("input", "output", 4),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                to_bytes(&[2.0_f32, 3.0, 4.0, 5.0]), // input
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                to_bytes(&[4.0_f32, 9.0, 16.0, 25.0]), // output = x*x
            ]]
        }),
        category: Some("math"),
    }
}

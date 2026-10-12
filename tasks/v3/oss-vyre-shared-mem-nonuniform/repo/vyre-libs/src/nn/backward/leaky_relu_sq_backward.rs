//! Backward for `leaky_relu_sq`: derivative of `max(αx, x)²`.
//!
//! For x≥0: d/dx = 2x. For x<0: d/dx = 2·(0.5x)·0.5 = 0.5x.
//! Branchless: `grad = dy * max(0.5*x, 2*x)`.

use vyre::ir::{Expr, Program};

use super::unary_f32::unary_f32_backward_program;

const OP_ID: &str = "vyre-libs::nn::leaky_relu_sq_backward";

/// Backward for leaky_relu_sq (F32).
#[must_use]
pub fn leaky_relu_sq_backward(input: &str, grad_out: &str, grad_in: &str, n: u32) -> Program {
    unary_f32_backward_program(OP_ID, input, grad_out, grad_in, n, |x| {
        // Branchless: for x>=0 -> 2x > 0.5x, for x<0 -> 0.5x > 2x.
        Expr::max(
            Expr::mul(Expr::f32(0.5), x.clone()),
            Expr::mul(Expr::f32(2.0), x),
        )
    })
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || leaky_relu_sq_backward("input", "grad_out", "grad_in", 4),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[2.0, -4.0, 0.0, 1.0]),
                to_f32(&[1.0, 1.0, 1.0, 1.0]),
                vec![0u8; 4 * 4],
            ]]
        }),
        expected_output: Some(|| {
            // x=2: max(1,4)=4; x=-4: max(-2,-8)=-2; x=0: 0; x=1: max(0.5,2)=2
            let out = [4.0_f32, -2.0, 0.0, 2.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    #[test]
    fn generated_leaky_relu_sq_backward_matches_scalar_reference() {
        let n = 512usize;
        let input = (0..n)
            .map(|i| ((i as i32 % 97) - 48) as f32 / 7.0)
            .collect::<Vec<_>>();
        let grad_out = (0..n)
            .map(|i| ((i as i32 % 31) - 15) as f32 / 5.0)
            .collect::<Vec<_>>();
        let program = leaky_relu_sq_backward("input", "grad_out", "grad_in", n as u32);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_f32_slice(&input)),
                Value::from(vyre_primitives::wire::pack_f32_slice(&grad_out)),
                Value::from(vec![0u8; n * core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: leaky_relu_sq_backward must execute in the reference interpreter.");
        let actual = vyre_primitives::wire::decode_f32_le_bytes_all(&outputs[0].to_bytes());
        for (index, ((actual, x), dy)) in actual
            .iter()
            .copied()
            .zip(input.iter().copied())
            .zip(grad_out.iter().copied())
            .enumerate()
        {
            let expected = dy * f32::max(0.5 * x, 2.0 * x);
            assert!(
                (actual - expected).abs() <= 1.0e-5,
                "generated leaky_relu_sq_backward mismatch at {index}: {actual} != {expected}"
            );
        }
    }
}

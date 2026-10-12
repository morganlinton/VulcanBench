//! LeakyReLU²: `y = leaky_relu(x, α=0.5)² = max(α·x, x)²`.
//!
//! Category A composition  -  element-wise `leaky_relu` (alpha=0.5)
//! followed by squaring (`mul self`). Used in the Parameter Golf
//! recipe as the MLP activation: hidden = leaky_relu_sq(linear(x)).

use vyre::ir::{Expr, Program};

const OP_ID: &str = "vyre-libs::nn::leaky_relu_sq";

fn leaky_relu_sq_expr(x: Expr) -> Expr {
    let half_x = Expr::mul(Expr::f32(0.5), x.clone());
    let leaky = Expr::max(half_x, x);
    Expr::mul(leaky.clone(), leaky)
}

/// Build a Program that applies `leaky_relu(x, 0.5)²` element-wise.
///
/// `input[n]` (F32, ReadOnly) → `output[n]` (F32).
///
/// For each element `x`:
///   `leaky = max(0.5 * x, x)`
///   `out   = leaky * leaky`
#[must_use]
pub fn leaky_relu_sq(input: &str, output: &str, n: u32) -> Program {
    super::unary::f32_unary_activation_program(OP_ID, input, output, n, leaky_relu_sq_expr)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || leaky_relu_sq("input", "output", 4),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                to_bytes(&[0.0_f32, 2.0, -4.0, 1.0]),
            ]]
        }),
        expected_output: Some(|| {
            // leaky_relu(0, 0.5)² = max(0, 0)² = 0
            // leaky_relu(2, 0.5)² = max(1, 2)² = 4
            // leaky_relu(-4, 0.5)² = max(-2, -4)² = (-2)² = 4
            // leaky_relu(1, 0.5)² = max(0.5, 1)² = 1
            let input = [0.0_f32, 2.0, -4.0, 1.0];
            let out: Vec<f32> = input.iter().map(|x| {
                let leaky = (0.5 * x).max(*x);
                leaky * leaky
            }).collect();
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn leaky_relu_sq_ref(x: f32) -> f32 {
        let leaky = (0.5 * x).max(x);
        leaky * leaky
    }

    #[test]
    fn leaky_relu_sq_nan_input_propagates_nan() {
        let input = [f32::NAN];
        let program = leaky_relu_sq("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: leaky_relu_sq must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(out[0].is_nan(), "leaky_relu_sq(NaN) must be NaN");
    }

    #[test]
    fn leaky_relu_sq_inf_inputs() {
        let program = leaky_relu_sq("input", "output", 2);
        // +Inf: max(0.5*Inf, Inf) = Inf, Inf*Inf = Inf
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::INFINITY, 0.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: leaky_relu_sq must not panic on +Inf input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], f32::INFINITY, "leaky_relu_sq(+Inf) must be +Inf");

        // -Inf: max(-0.5*Inf, -Inf) = max(-Inf, -Inf) = -Inf, (-Inf)*(-Inf) = +Inf
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::NEG_INFINITY, 0.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: leaky_relu_sq must not panic on -Inf input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(
            out[0],
            f32::INFINITY,
            "leaky_relu_sq(-Inf) must be +Inf (square of negative infinity)"
        );
    }

    #[test]
    fn leaky_relu_sq_negative_zero_vs_positive_zero() {
        let program = leaky_relu_sq("input", "output", 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[0.0f32, -0.0f32])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: leaky_relu_sq must handle -0.0");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0].to_bits(), 0.0f32.to_bits());
        assert_eq!(
            out[1].to_bits(),
            0.0f32.to_bits(),
            "leaky_relu_sq(-0.0) must be +0.0"
        );
    }

    #[test]
    fn leaky_relu_sq_subnormal_input() {
        let sub = f32::from_bits(1);
        let program = leaky_relu_sq("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&[sub])), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: leaky_relu_sq must not panic on subnormal input");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = leaky_relu_sq_ref(sub);
        assert!(
            (out[0] - expected).abs() <= 1.0e-6,
            "leaky_relu_sq(subnormal) mismatch"
        );
    }

    #[test]
    fn generated_leaky_relu_sq_matches_scalar_reference() {
        let input = (0..2048u32)
            .map(|i| ((i as f32) * 0.031).cos() * 8.0 - 4.0)
            .collect::<Vec<_>>();
        let program = leaky_relu_sq("input", "output", input.len() as u32);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&input)),
                Value::from(vec![0u8; input.len() * core::mem::size_of::<f32>()]),
            ],
        )
        .expect("Fix: generated leaky_relu_sq corpus must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        for (index, (actual, expected)) in out
            .iter()
            .copied()
            .zip(input.iter().copied().map(leaky_relu_sq_ref))
            .enumerate()
        {
            assert!(
                (actual - expected).abs() <= 1.0e-5,
                "generated leaky_relu_sq mismatch at {index}: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn leaky_relu_sq_all_zeros() {
        let input = [0.0f32; 4];
        let program = leaky_relu_sq("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: leaky_relu_sq all-zeros must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn leaky_relu_sq_all_ones() {
        let input = [1.0f32; 4];
        let program = leaky_relu_sq("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: leaky_relu_sq all-ones must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![1.0; 4]);
    }

    #[test]
    fn leaky_relu_sq_all_max_f32() {
        let input = [f32::MAX; 4];
        let program = leaky_relu_sq("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: leaky_relu_sq all-max-f32 must not panic");
        let out = decode_f32(&outputs[0].to_bytes());
        for (i, &v) in out.iter().enumerate() {
            assert_eq!(
                v,
                f32::INFINITY,
                "leaky_relu_sq(f32::MAX) must overflow to +Inf at {i}: got {v}"
            );
        }
    }

    #[test]
    fn leaky_relu_sq_single_element() {
        let input = [-3.0f32];
        let program = leaky_relu_sq("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: leaky_relu_sq single element must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = leaky_relu_sq_ref(-3.0);
        assert!(
            (out[0] - expected).abs() <= 1.0e-5,
            "leaky_relu_sq single element mismatch"
        );
    }

    #[test]
    fn leaky_relu_sq_empty_tensor() {
        let program = leaky_relu_sq("input", "output", 0);
        let outputs =
            vyre_reference::reference_eval(&program, &[Value::from(vec![]), Value::from(vec![])])
                .expect("Fix: leaky_relu_sq n=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn leaky_relu_sq_output_is_nonnegative(x in prop::num::f32::NORMAL) {
            let program = leaky_relu_sq("input", "output", 1);
            let outputs = vyre_reference::reference_eval(
                &program,
                &[Value::from(f32_bytes(&[x])), Value::from(vec![0u8; 4])],
            )
            .expect("Fix: leaky_relu_sq must not panic on finite input");
            let out = decode_f32(&outputs[0].to_bytes())[0];
            if x.is_nan() {
                prop_assert!(out.is_nan());
            } else {
                prop_assert!(out >= 0.0 || out.is_nan(), "leaky_relu_sq(x) must be >= 0 or NaN, got {out}");
            }
        }
    }
}

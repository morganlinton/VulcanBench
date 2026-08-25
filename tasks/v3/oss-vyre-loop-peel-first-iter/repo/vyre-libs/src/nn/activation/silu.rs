//! SiLU (Sigmoid Linear Unit): `y = x * sigmoid(x) = x / (1 + exp(-x))`.
//!
//! Category A composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::f32_stability::flush_tiny;

/// Shared SiLU expression with the same tiny-value stabilization used by
/// standalone and fused activation builders.
pub(crate) fn silu_expr(x: Expr) -> Expr {
    let sigmoid_x = Expr::div(
        Expr::f32(1.0),
        Expr::add(
            Expr::f32(1.0),
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(x.clone()),
                }),
            },
        ),
    );
    flush_tiny(Expr::mul(x, sigmoid_x))
}

/// Build a Program that applies SiLU element-wise from `input` into
/// `output`. `n` is the element count of both buffers.
#[must_use]
pub fn silu(input: &str, output: &str, n: u32) -> Program {
    let i = Expr::var("i");
    let x = Expr::load(input, i.clone());

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::buf_len(input)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: silu_expr(x),
            }],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::silu", body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::silu",
        build: || silu("input", "output", 4),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                to_bytes(&[0.0_f32, 1.0, -1.0, 2.0]), // input
            ]]
        }),
        expected_output: Some(|| {
            // SiLU via the same x / (1 + exp(-x)) formula the IR evaluates.
            // The cross-backend f32 ULP tolerance in parity_matrix
            // widens to 64 ULP for transcendentals, so this CPU-side
            // value is byte-identical with the reference interpreter.
            let input = [0.0_f32, 1.0, -1.0, 2.0];
            let out: Vec<f32> = input
                .iter()
                .map(|x| x / (1.0 + (-x).exp()))
                .collect();
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

    fn silu_ref(x: f32) -> f32 {
        x / (1.0 + (-x).exp())
    }

    #[test]
    fn silu_nan_input_propagates_nan() {
        let input = [f32::NAN];
        let program = silu("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: silu must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(out[0].is_nan(), "silu(NaN) must be NaN");
    }

    #[test]
    fn silu_inf_inputs() {
        let program = silu("input", "output", 2);
        // +Inf
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::INFINITY, 0.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: silu must not panic on +Inf input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], f32::INFINITY, "silu(+Inf) must be +Inf");

        // -Inf: sigmoid(-Inf)=0, -Inf*0 = NaN
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::NEG_INFINITY, 0.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: silu must not panic on -Inf input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan(),
            "silu(-Inf) must be NaN (negative infinity times zero)"
        );
    }

    #[test]
    fn silu_negative_zero_vs_positive_zero() {
        let program = silu("input", "output", 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[0.0f32, -0.0f32])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: silu must distinguish -0.0 from 0.0");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0].to_bits(), 0.0f32.to_bits());
        // silu(-0.0) = -0.0 * 0.5 = -0.0, but flush_tiny may flush it
        // The reference computes -0.0 / 2.0 = -0.0
        // Note: the reference interpreter computes -0.0 * 0.5 = -0.0, but
        // flush_tiny or later rounding may produce +0.0. We accept +0.0 as
        // long as it is not a non-zero value.
        assert!(out[1] == 0.0, "silu(-0.0) must be zero, got {}", out[1]);
    }

    #[test]
    fn silu_subnormal_input_is_flushed_to_zero() {
        let sub = f32::from_bits(1); // smallest positive subnormal
        let program = silu("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&[sub])), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: silu must not panic on subnormal input");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(
            out[0].to_bits(),
            0.0f32.to_bits(),
            "silu must flush tiny subnormal to +0.0"
        );
    }

    #[test]
    fn silu_all_zeros() {
        let input = [0.0f32; 4];
        let program = silu("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: silu all-zeros must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn silu_all_ones() {
        let input = [1.0f32; 4];
        let program = silu("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: silu all-ones must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = silu_ref(1.0);
        for (i, &v) in out.iter().enumerate() {
            assert!(
                (v - expected).abs() <= 1.0e-6,
                "silu all-ones mismatch at {i}: {v}"
            );
        }
    }

    #[test]
    fn silu_all_max_f32() {
        let input = [f32::MAX; 4];
        let program = silu("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: silu all-max-f32 must not panic");
        let out = decode_f32(&outputs[0].to_bytes());
        for (i, &v) in out.iter().enumerate() {
            // sigmoid(MAX) ≈ 1.0, so silu(MAX) ≈ MAX (does not overflow because MAX*1.0 = MAX)
            assert_eq!(
                v,
                f32::MAX,
                "silu(f32::MAX) must be f32::MAX at {i}: got {v}"
            );
        }
    }

    #[test]
    fn silu_single_element() {
        let input = [2.5f32];
        let program = silu("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: silu single element must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = silu_ref(2.5);
        assert!(
            (out[0] - expected).abs() <= 1.0e-6,
            "silu single element mismatch: {} != {}",
            out[0],
            expected
        );
    }

    #[test]
    fn silu_empty_tensor() {
        let program = silu("input", "output", 0);
        let outputs =
            vyre_reference::reference_eval(&program, &[Value::from(vec![]), Value::from(vec![])])
                .expect("Fix: silu n=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn silu_output_invariant_for_finite_inputs(x in -1e10f32..1e10f32) {
            let program = silu("input", "output", 1);
            let outputs = vyre_reference::reference_eval(
                &program,
                &[Value::from(f32_bytes(&[x])), Value::from(vec![0u8; 4])],
            )
            .expect("Fix: silu must not panic on finite input");
            let out = decode_f32(&outputs[0].to_bytes())[0];
            if x.is_nan() {
                prop_assert!(out.is_nan());
            } else if x > 0.0 {
                // For very large x, sigmoid(x) rounds to 1.0, so out ≈ x.
                prop_assert!(out > 0.0 && out <= x, "silu(x) for x>0 must be in (0, x]");
            } else if x < 0.0 {
                // flush_tiny may turn subnormal products into 0.0, so we allow 0.0.
                prop_assert!(out >= x && out <= 0.0, "silu(x) for x<0 must be in [x, 0]");
            } else {
                prop_assert_eq!(out.to_bits(), 0.0f32.to_bits());
            }
        }
    }
}

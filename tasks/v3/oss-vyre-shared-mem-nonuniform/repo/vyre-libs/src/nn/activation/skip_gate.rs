//! Skip gate: sigmoid-gated U-Net skip connection.
//!
//! `out = sigmoid(g) * branch + (1 - sigmoid(g)) * skip`
//!
//! Category A composition  -  sigmoid + mul + add. Used in the recipe
//! for U-Net skip connections between encoder and decoder layers.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::f32_stability::flush_tiny;

const OP_ID: &str = "vyre-libs::nn::skip_gate";

/// Build a Program for sigmoid-gated skip connection.
///
/// `gate[n]` (F32)  -  raw gate logits (sigmoid applied here).
/// `branch[n]` (F32)  -  output of the transformer block.
/// `skip[n]` (F32)  -  skip connection from encoder.
/// `output[n]` (F32)  -  gated combination.
#[must_use]
pub fn skip_gate(gate: &str, branch: &str, skip: &str, output: &str, n: u32) -> Program {
    let i = Expr::var("i");
    let g_raw = Expr::load(gate, i.clone());
    let b = Expr::load(branch, i.clone());
    let s = Expr::load(skip, i.clone());

    // sigmoid(g) = 1 / (1 + exp(-g))
    let sigmoid_g = Expr::div(
        Expr::f32(1.0),
        Expr::add(
            Expr::f32(1.0),
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(g_raw),
                }),
            },
        ),
    );

    // out = sig * branch + (1 - sig) * skip
    let result = Expr::add(
        Expr::mul(sigmoid_g.clone(), b),
        Expr::mul(Expr::sub(Expr::f32(1.0), sigmoid_g), s),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: flush_tiny(result),
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(gate, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(branch, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(skip, 2, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || skip_gate("gate", "branch", "skip", "output", 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[0.0, 100.0]),  // gate logits (sigmoid(0)=0.5, sigmoid(100)≈1)
                to_f32(&[10.0, 20.0]),  // branch
                to_f32(&[30.0, 40.0]),  // skip
            ]]
        }),
        expected_output: Some(|| {
            fn sigmoid(x: f32) -> f32 { 1.0 / (1.0 + (-x).exp()) }
            let out = [
                sigmoid(0.0) * 10.0 + (1.0 - sigmoid(0.0)) * 30.0,   // 0.5*10 + 0.5*30 = 20
                sigmoid(100.0) * 20.0 + (1.0 - sigmoid(100.0)) * 40.0, // ≈ 20
            ];
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

    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }

    #[test]
    fn skip_gate_nan_in_gate_propagates_nan() {
        let gate = [f32::NAN];
        let branch = [1.0f32];
        let skip = [2.0f32];
        let program = skip_gate("gate", "branch", "skip", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&gate)),
                Value::from(f32_bytes(&branch)),
                Value::from(f32_bytes(&skip)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: skip_gate must not panic on NaN gate");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(out[0].is_nan(), "skip_gate(NaN gate) must be NaN");
    }

    #[test]
    fn skip_gate_inf_gate_selects_branch_or_skip() {
        let program = skip_gate("gate", "branch", "skip", "output", 2);
        // +Inf gate → sigmoid(+Inf)=1 → branch
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::INFINITY, 0.0])),
                Value::from(f32_bytes(&[10.0, 20.0])),
                Value::from(f32_bytes(&[30.0, 40.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: skip_gate must not panic on +Inf gate");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 10.0, "skip_gate(+Inf gate) must select branch");

        // -Inf gate → sigmoid(-Inf)=0 → skip
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[f32::NEG_INFINITY, 0.0])),
                Value::from(f32_bytes(&[10.0, 20.0])),
                Value::from(f32_bytes(&[30.0, 40.0])),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: skip_gate must not panic on -Inf gate");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 30.0, "skip_gate(-Inf gate) must select skip");
    }

    #[test]
    fn skip_gate_nan_in_branch_propagates_nan() {
        let gate = [0.0f32];
        let branch = [f32::NAN];
        let skip = [2.0f32];
        let program = skip_gate("gate", "branch", "skip", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&gate)),
                Value::from(f32_bytes(&branch)),
                Value::from(f32_bytes(&skip)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: skip_gate must not panic on NaN branch");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan(),
            "skip_gate(NaN branch) must be NaN (sigmoid(0)=0.5, 0.5*NaN = NaN)"
        );
    }

    #[test]
    fn skip_gate_nan_in_skip_propagates_nan() {
        let gate = [0.0f32];
        let branch = [1.0f32];
        let skip = [f32::NAN];
        let program = skip_gate("gate", "branch", "skip", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&gate)),
                Value::from(f32_bytes(&branch)),
                Value::from(f32_bytes(&skip)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: skip_gate must not panic on NaN skip");
        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            out[0].is_nan(),
            "skip_gate(NaN skip) must be NaN (0.5*NaN = NaN)"
        );
    }

    #[test]
    fn skip_gate_all_zeros() {
        let program = skip_gate("gate", "branch", "skip", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[0.0; 4])),
                Value::from(f32_bytes(&[0.0; 4])),
                Value::from(f32_bytes(&[0.0; 4])),
                Value::from(vec![0u8; 16]),
            ],
        )
        .expect("Fix: skip_gate all-zeros must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out, vec![0.0; 4]);
    }

    #[test]
    fn skip_gate_all_ones() {
        let program = skip_gate("gate", "branch", "skip", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[1.0; 4])),
                Value::from(f32_bytes(&[1.0; 4])),
                Value::from(f32_bytes(&[1.0; 4])),
                Value::from(vec![0u8; 16]),
            ],
        )
        .expect("Fix: skip_gate all-ones must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let s = sigmoid(1.0);
        let expected = s * 1.0 + (1.0 - s) * 1.0;
        for (i, &v) in out.iter().enumerate() {
            assert!(
                (v - expected).abs() <= 1.0e-5,
                "skip_gate all-ones mismatch at {i}: {v}"
            );
        }
    }

    #[test]
    fn skip_gate_single_element() {
        let program = skip_gate("gate", "branch", "skip", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[2.0])),
                Value::from(f32_bytes(&[10.0])),
                Value::from(f32_bytes(&[20.0])),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: skip_gate single element must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        let s = sigmoid(2.0);
        let expected = s * 10.0 + (1.0 - s) * 20.0;
        assert!(
            (out[0] - expected).abs() <= 1.0e-5,
            "skip_gate single element mismatch"
        );
    }

    #[test]
    fn skip_gate_empty_tensor() {
        let program = skip_gate("gate", "branch", "skip", "output", 0);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vec![]),
                Value::from(vec![]),
                Value::from(vec![]),
                Value::from(vec![]),
            ],
        )
        .expect("Fix: skip_gate n=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
    }
}

//! Backward for `parallel_residual_block`:
//!
//! Forward: `out = x + attn_out + mlp_out`
//! Backward: `grad_x = grad_attn = grad_mlp = grad_out` (addition broadcast).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::residual_block_backward";

/// Backward for parallel_residual_block (F32).
///
/// Since forward is just addition, all three input gradients equal grad_out.
/// This op copies grad_out → grad_x, grad_attn, grad_mlp.
#[must_use]
pub fn residual_block_backward(
    grad_out: &str,
    grad_x: &str,
    grad_attn: &str,
    grad_mlp: &str,
    n: u32,
) -> Program {
    let i = Expr::var("i");
    let dy = Expr::load(grad_out, i.clone());

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::Store {
                    buffer: grad_x.into(),
                    index: i.clone(),
                    value: dy.clone(),
                },
                Node::Store {
                    buffer: grad_attn.into(),
                    index: i.clone(),
                    value: dy.clone(),
                },
                Node::Store {
                    buffer: grad_mlp.into(),
                    index: i,
                    value: dy,
                },
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(grad_out, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(grad_x, 1, DataType::F32).with_count(n),
            BufferDecl::storage(grad_attn, 2, BufferAccess::ReadWrite, DataType::F32).with_count(n),
            BufferDecl::storage(grad_mlp, 3, BufferAccess::ReadWrite, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || residual_block_backward("grad_out", "grad_x", "grad_attn", "grad_mlp", 4),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0, 3.0, 4.0]),
                vec![0u8; 4 * 4], // grad_attn
                vec![0u8; 4 * 4], // grad_mlp
            ]]
        }),
        expected_output: Some(|| {
            // All three live-outs = copy of grad_out.
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            let expected = to_f32(&[1.0, 2.0, 3.0, 4.0]);
            vec![vec![expected.clone(), expected.clone(), expected]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::residual_block_backward;
    use vyre_reference::value::Value;

    fn f32_bytes(values: &[f32]) -> Vec<u8> {
        vyre_primitives::wire::pack_f32_slice(values)
    }

    #[test]
    fn reference_outputs_all_residual_gradient_liveouts() {
        let program = residual_block_backward("grad_out", "grad_x", "grad_attn", "grad_mlp", 4);
        let expected = f32_bytes(&[1.0, 2.0, 3.0, 4.0]);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(expected.clone()),
                Value::from(vec![0_u8; 16]),
                Value::from(vec![0_u8; 16]),
            ],
        )
        .expect("Fix: residual_block_backward must satisfy the one-output plus ReadWrite live-out IR contract.");

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].to_bytes(), expected);
        assert_eq!(outputs[1].to_bytes(), expected);
        assert_eq!(outputs[2].to_bytes(), expected);
    }
}

//! Backward for `qk_gain`: `grad_q = grad_out * gain[h]`.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::qk_gain_backward";

/// Backward for qk_gain (F32). Produces grad_q.
#[must_use]
pub fn qk_gain_backward(
    gain: &str,
    grad_out: &str,
    grad_q: &str,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
) -> Program {
    let total = num_heads * seq_len * head_dim;
    let per_head = seq_len * head_dim;

    let i = Expr::var("i");
    let head_idx = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(i.clone()),
        right: Box::new(Expr::u32(per_head)),
    };
    let grad = Expr::mul(Expr::load(grad_out, i.clone()), Expr::load(gain, head_idx));

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(total)),
            vec![Node::Store {
                buffer: grad_q.into(),
                index: i,
                value: grad,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(gain, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_heads),
            BufferDecl::storage(grad_out, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(total),
            BufferDecl::output(grad_q, 2, DataType::F32).with_count(total),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || qk_gain_backward("gain", "grad_out", "grad_q", 2, 1, 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[5.25, 3.0]),
                to_f32(&[1.0, 1.0, 1.0, 1.0]),
                vec![0u8; 4 * 4],
            ]]
        }),
        expected_output: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![to_f32(&[5.25, 5.25, 3.0, 3.0])]]
        }),
        category: Some("nn"),
    }
}

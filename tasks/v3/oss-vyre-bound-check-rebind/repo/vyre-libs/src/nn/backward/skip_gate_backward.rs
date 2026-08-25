//! Backward for `skip_gate`:
//!
//! `grad_gate = grad_out * σ(g) * (1-σ(g)) * (branch - skip)`

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::skip_gate_backward";

/// Backward for skip_gate (F32). Produces grad_gate.
#[must_use]
pub fn skip_gate_backward(
    gate: &str,
    branch: &str,
    skip: &str,
    grad_out: &str,
    grad_gate: &str,
    n: u32,
) -> Program {
    let i = Expr::var("i");
    let g = Expr::load(gate, i.clone());
    let b = Expr::load(branch, i.clone());
    let s = Expr::load(skip, i.clone());
    let dy = Expr::load(grad_out, i.clone());

    let sig = Expr::div(
        Expr::f32(1.0),
        Expr::add(
            Expr::f32(1.0),
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(g),
                }),
            },
        ),
    );
    let grad = Expr::mul(
        dy,
        Expr::mul(
            Expr::mul(sig.clone(), Expr::sub(Expr::f32(1.0), sig)),
            Expr::sub(b, s),
        ),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: grad_gate.into(),
                index: i,
                value: grad,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(gate, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(branch, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(skip, 2, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(grad_out, 3, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(grad_gate, 4, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || skip_gate_backward("gate", "branch", "skip", "grad_out", "grad_gate", 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[0.0, 100.0]),
                to_f32(&[10.0, 20.0]),
                to_f32(&[30.0, 40.0]),
                to_f32(&[1.0, 1.0]),
                vec![0u8; 4 * 2],
            ]]
        }),
        expected_output: Some(|| {
            fn sigmoid(x: f32) -> f32 { 1.0 / (1.0 + (-x).exp()) }
            let out = [
                sigmoid(0.0) * (1.0 - sigmoid(0.0)) * (10.0 - 30.0),
                sigmoid(100.0) * (1.0 - sigmoid(100.0)) * (20.0 - 40.0),
            ];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

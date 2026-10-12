//! Shared Muon optimizer step builder.
//!
//! `muon_update` and `muoneq_r` differ only in the scalar applied to the
//! Nesterov update. The momentum/update IR body is centralized here so optimizer
//! variants cannot drift.

use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::nn::f32_stability::flush_tiny;

/// Build a Muon-style F32 optimizer step.
#[must_use]
pub(crate) fn muon_step_program(
    op_id: &'static str,
    params: &str,
    grads: &str,
    momentum_buf: &str,
    output: &str,
    n: u32,
    lr_scale: f32,
    momentum: f32,
) -> Program {
    let i = Expr::var("i");
    let g = flush_tiny(Expr::load(grads, i.clone()));
    let m = flush_tiny(Expr::load(momentum_buf, i.clone()));
    let p = flush_tiny(Expr::load(params, i.clone()));
    let new_m = Expr::add(Expr::mul(Expr::f32(momentum), m), g.clone());
    let nesterov = Expr::add(g, Expr::mul(Expr::f32(momentum), new_m.clone()));
    let new_p = Expr::sub(p, Expr::mul(Expr::f32(lr_scale), nesterov));
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::Store {
                    buffer: momentum_buf.into(),
                    index: i.clone(),
                    value: flush_tiny(new_m),
                },
                Node::Store {
                    buffer: output.into(),
                    index: i,
                    value: flush_tiny(new_p),
                },
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(params, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(grads, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(momentum_buf, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(n),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(op_id, body)],
    )
}

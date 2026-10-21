//! AdamW optimizer step (F32).
//!
//! `m = β₁*m + (1-β₁)*g`
//! `v = β₂*v + (1-β₂)*g²`
//! `θ = θ * (1 - lr*wd) - lr * m̂ / (√v̂ + ε)`

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::f32_stability::flush_tiny;

const OP_ID: &str = "vyre-libs::optim::adamw_step";

/// Build a single AdamW step (F32).
///
/// `params[n]` (RW), `grads[n]` (RO), `m[n]` (RW), `v[n]` (RW).
/// Hyperparams baked as constants.
#[must_use]
pub fn adamw_step(
    params: &str,
    grads: &str,
    m_buf: &str,
    v_buf: &str,
    n: u32,
    lr: f32,
    beta1: f32,
    beta2: f32,
    eps: f32,
    wd: f32,
) -> Program {
    let i = Expr::var("i");
    let g = flush_tiny(Expr::load(grads, i.clone()));
    let m = flush_tiny(Expr::load(m_buf, i.clone()));
    let v = flush_tiny(Expr::load(v_buf, i.clone()));
    let p = flush_tiny(Expr::load(params, i.clone()));

    // m = β₁*m + (1-β₁)*g
    let new_m = Expr::add(
        Expr::mul(Expr::f32(beta1), m),
        Expr::mul(Expr::f32(1.0 - beta1), g.clone()),
    );

    // v = β₂*v + (1-β₂)*g²
    let new_v = Expr::add(
        Expr::mul(Expr::f32(beta2), v),
        Expr::mul(Expr::f32(1.0 - beta2), Expr::mul(g.clone(), g)),
    );

    // weight decay + adam update: θ = θ*(1-lr*wd) - lr * m / (√v + ε)
    // (bias correction omitted  -  recipe uses β-schedule instead)
    let decayed = Expr::mul(p, Expr::f32(1.0 - lr * wd));
    let denom = Expr::add(
        Expr::UnOp {
            op: UnOp::Sqrt,
            operand: Box::new(new_v.clone()),
        },
        Expr::f32(eps),
    );
    let new_p = Expr::sub(
        decayed,
        Expr::mul(Expr::f32(lr), Expr::div(new_m.clone(), denom)),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![
                Node::Store {
                    buffer: m_buf.into(),
                    index: i.clone(),
                    value: flush_tiny(new_m),
                },
                Node::Store {
                    buffer: v_buf.into(),
                    index: i.clone(),
                    value: flush_tiny(new_v),
                },
                Node::Store {
                    buffer: params.into(),
                    index: i,
                    value: flush_tiny(new_p),
                },
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(params, 0, BufferAccess::ReadWrite, DataType::F32).with_count(n),
            BufferDecl::storage(grads, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(m_buf, 2, BufferAccess::ReadWrite, DataType::F32).with_count(n),
            BufferDecl::storage(v_buf, 3, BufferAccess::ReadWrite, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || adamw_step("params", "grads", "m", "v", 2, 0.001, 0.9, 0.999, 1e-8, 0.01),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0]),          // params
                to_f32(&[0.1, 0.2]),          // grads
                to_f32(&[0.0, 0.0]),          // m (first step)
                to_f32(&[0.0, 0.0]),          // v (first step)
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![
                vec![215, 232, 126, 63, 24, 116, 255, 63],
                vec![13, 215, 35, 60, 13, 215, 163, 60],
                vec![31, 197, 39, 55, 31, 197, 39, 56],
            ]]
        }),
        category: Some("nn"),
    }
}

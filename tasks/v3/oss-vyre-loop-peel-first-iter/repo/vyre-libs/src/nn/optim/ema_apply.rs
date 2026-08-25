//! EMA update: `ema = decay * ema + (1 - decay) * theta`.
//!
//! Category A  -  element-wise weighted average. Recipe decay=0.9965.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::f32_stability::flush_tiny;

const OP_ID: &str = "vyre-libs::optim::ema_apply";

/// Build a Program for EMA update in-place (F32).
///
/// `ema[n]` (RW)  -  running average.
/// `theta[n]` (RO)  -  current weights.
/// `decay`  -  scalar, baked as constant.
#[must_use]
pub fn ema_apply(ema: &str, theta: &str, n: u32, decay: f32) -> Program {
    let i = Expr::var("i");
    let ema_val = Expr::load(ema, i.clone());
    let theta_val = Expr::load(theta, i.clone());

    // ema = decay * ema + (1 - decay) * theta
    let updated = Expr::add(
        Expr::mul(Expr::f32(decay), ema_val),
        Expr::mul(Expr::f32(1.0 - decay), theta_val),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: ema.into(),
                index: i,
                value: flush_tiny(updated),
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(ema, 0, BufferAccess::ReadWrite, DataType::F32).with_count(n),
            BufferDecl::storage(theta, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || ema_apply("ema", "theta", 4, 0.9),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[10.0, 20.0, 30.0, 40.0]),  // ema
                to_f32(&[11.0, 21.0, 31.0, 41.0]),  // theta
            ]]
        }),
        expected_output: Some(|| {
            let decay = 0.9_f32;
            let ema = [10.0_f32, 20.0, 30.0, 40.0];
            let theta = [11.0_f32, 21.0, 31.0, 41.0];
            let out: Vec<f32> = ema.iter().zip(theta.iter())
                .map(|(e, t)| decay * e + (1.0 - decay) * t)
                .collect();
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

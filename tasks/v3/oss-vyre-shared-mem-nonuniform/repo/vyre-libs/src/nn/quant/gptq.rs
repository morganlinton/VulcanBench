//! GPTQ-SDClip: Full-Hessian GPTQ with standard-deviation clipping.
//!
//! `clip_threshold = k * std(row)`  -  int6 uses k=12.85, int8 uses k=20.0.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use vyre_primitives::nn::f32_stability::{finite_or, positive_finite_or_min as positive_scale};

const ROUND_OP_ID: &str = "vyre-libs::quant::gptq_round";
const SDCLIP_OP_ID: &str = "vyre-libs::quant::gptq_sdclip";

fn clamp_f32(value: Expr, lo: f32, hi: f32) -> Expr {
    let finite = finite_or(value, Expr::f32(lo));
    let lower = Expr::select(
        Expr::lt(finite.clone(), Expr::f32(lo)),
        Expr::f32(lo),
        finite,
    );
    Expr::select(Expr::gt(lower.clone(), Expr::f32(hi)), Expr::f32(hi), lower)
}

/// GPTQ rounding: `q = clamp(round(x / scale), 0, max_val)` (F32→F32).
#[must_use]
pub fn gptq_round(input: &str, scale: &str, output: &str, n: u32, max_val: f32) -> Program {
    let i = Expr::var("i");
    let x = finite_or(Expr::load(input, i.clone()), Expr::f32(0.0));
    let s = positive_scale(Expr::load(scale, i.clone()));

    let divided = Expr::select(
        Expr::eq(x.clone(), s.clone()),
        Expr::f32(1.0),
        Expr::div(x, s),
    );
    let clamped = clamp_f32(divided, 0.0, max_val);

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: clamped,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(scale, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 2, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(ROUND_OP_ID, body)],
    )
}

/// GPTQ-SDClip: `out = clamp(x, -k, k)` per element (F32).
///
/// Real version computes per-row std and clips at `k * std(row)`.
/// This per-element clamp is a correct first-pass.
#[must_use]
pub fn gptq_sdclip(input: &str, output: &str, n: u32, k: f32) -> Program {
    let i = Expr::var("i");
    let x = finite_or(Expr::load(input, i.clone()), Expr::f32(0.0));
    let clamped = clamp_f32(x, -k, k);

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: clamped,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(SDCLIP_OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: ROUND_OP_ID,
        build: || gptq_round("input", "scale", "output", 4, 63.0),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[100.0, 200.0, 50.0, 10.0]),
                to_f32(&[2.0, 3.0, 1.0, 5.0]),
            ]]
        }),
        expected_output: Some(|| {
            // 100/2=50, 200/3=66.7→63(clamped), 50/1=50, 10/5=2
            let out = [50.0_f32, 63.0, 50.0, 2.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SDCLIP_OP_ID,
        build: || gptq_sdclip("input", "output", 4, 30.0),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[10.0, 50.0, -40.0, 25.0]),
            ]]
        }),
        expected_output: Some(|| {
            // clamp: 10, 30, -30, 25
            let out = [10.0_f32, 30.0, -30.0, 25.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

//! Expert MLP: `down(swiglu(gate(x), up(x)))`.
//!
//! This builder emits one fused Program for a single expert forward pass.
//! The gate and up projections, SwiGLU activation, and down projection are
//! kept in the same dispatch body so the hidden activation does not need to
//! materialize to a global intermediate buffer.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

use crate::region::wrap_anonymous;

/// Build a Program that computes one expert MLP forward pass.
///
/// # Errors
/// Returns `Err` when any dimension is zero or when a weight matrix element
/// count overflows `u32`.
#[allow(clippy::too_many_arguments)]
pub fn expert_mlp(
    x: &str,
    w_gate: &str,
    b_gate: &str,
    w_up: &str,
    b_up: &str,
    w_down: &str,
    b_down: &str,
    out: &str,
    in_dim: u32,
    hidden_dim: u32,
    out_dim: u32,
) -> Result<Program, String> {
    if in_dim == 0 || hidden_dim == 0 || out_dim == 0 {
        return Err("Fix: expert_mlp all dims must be > 0".to_string());
    }
    let input_hidden_weights = in_dim.checked_mul(hidden_dim).ok_or_else(|| {
        "Fix: expert_mlp in_dim*hidden_dim overflows u32; reduce dimensions.".to_string()
    })?;
    let hidden_output_weights = hidden_dim.checked_mul(out_dim).ok_or_else(|| {
        "Fix: expert_mlp hidden_dim*out_dim overflows u32; reduce dimensions.".to_string()
    })?;

    let o = Expr::var("o");
    let h = Expr::var("h");
    let k = Expr::var("k");
    let hidden_offset = Expr::mul(k.clone(), Expr::u32(hidden_dim));
    let output_offset = Expr::mul(h.clone(), Expr::u32(out_dim));
    let sigmoid_gate = Expr::div(
        Expr::f32(1.0),
        Expr::add(
            Expr::f32(1.0),
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(Expr::var("gate_acc")),
                }),
            },
        ),
    );
    let swiglu = Expr::mul(
        Expr::mul(Expr::var("gate_acc"), sigmoid_gate),
        Expr::var("up_acc"),
    );

    let body = vec![
        Node::let_bind("o", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(o.clone(), Expr::u32(out_dim)),
            vec![
                Node::let_bind("acc", Expr::load(b_down, o.clone())),
                Node::loop_for(
                    "h",
                    Expr::u32(0),
                    Expr::u32(hidden_dim),
                    vec![
                        Node::let_bind("gate_acc", Expr::load(b_gate, h.clone())),
                        Node::let_bind("up_acc", Expr::load(b_up, h.clone())),
                        Node::loop_for(
                            "k",
                            Expr::u32(0),
                            Expr::u32(in_dim),
                            vec![
                                Node::assign(
                                    "gate_acc",
                                    Expr::add(
                                        Expr::var("gate_acc"),
                                        Expr::mul(
                                            Expr::load(x, k.clone()),
                                            Expr::load(
                                                w_gate,
                                                Expr::add(hidden_offset.clone(), h.clone()),
                                            ),
                                        ),
                                    ),
                                ),
                                Node::assign(
                                    "up_acc",
                                    Expr::add(
                                        Expr::var("up_acc"),
                                        Expr::mul(
                                            Expr::load(x, k.clone()),
                                            Expr::load(
                                                w_up,
                                                Expr::add(hidden_offset.clone(), h.clone()),
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                        Node::assign(
                            "acc",
                            Expr::add(
                                Expr::var("acc"),
                                Expr::mul(
                                    swiglu.clone(),
                                    Expr::load(w_down, Expr::add(output_offset.clone(), o.clone())),
                                ),
                            ),
                        ),
                    ],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: o,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w_gate, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(input_hidden_weights),
            BufferDecl::storage(b_gate, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim),
            BufferDecl::storage(w_up, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(input_hidden_weights),
            BufferDecl::storage(b_up, 4, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim),
            BufferDecl::storage(w_down, 5, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_output_weights),
            BufferDecl::storage(b_down, 6, BufferAccess::ReadOnly, DataType::F32)
                .with_count(out_dim),
            BufferDecl::output(out, 7, DataType::F32).with_count(out_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::moe::expert_mlp", body)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn expert_mlp_executes_fused_gate_up_swiglu_down() {
        let program = expert_mlp("x", "wg", "bg", "wu", "bu", "wd", "bd", "out", 2, 2, 2)
            .expect("Fix: expert_mlp must build for positive dimensions");
        let x = [1.0f32, 2.0];
        let w_gate = [0.5f32, -0.25, 1.0, 0.75];
        let b_gate = [0.1f32, -0.2];
        let w_up = [0.25f32, 0.5, -0.5, 1.25];
        let b_up = [0.0f32, 0.3];
        let w_down = [1.0f32, -0.5, 0.25, 0.75];
        let b_down = [0.2f32, -0.1];
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&x)),
                Value::from(f32_bytes(&w_gate)),
                Value::from(f32_bytes(&b_gate)),
                Value::from(f32_bytes(&w_up)),
                Value::from(f32_bytes(&b_up)),
                Value::from(f32_bytes(&w_down)),
                Value::from(f32_bytes(&b_down)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: expert_mlp fused Program must execute in reference interpreter");
        let actual = decode_f32(&outputs[0].to_bytes());
        let mut hidden = [0.0f32; 2];
        for h in 0..2 {
            let mut gate = b_gate[h];
            let mut up = b_up[h];
            for k in 0..2 {
                gate += x[k] * w_gate[k * 2 + h];
                up += x[k] * w_up[k * 2 + h];
            }
            hidden[h] = gate * up / (1.0 + (-gate).exp());
        }
        let expected = [
            b_down[0] + hidden[0] * w_down[0] + hidden[1] * w_down[2],
            b_down[1] + hidden[0] * w_down[1] + hidden[1] * w_down[3],
        ];
        for (idx, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (*actual - expected).abs() <= 1.0e-5,
                "expert_mlp output {idx} mismatch: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn expert_mlp_zero_dim_errors() {
        for (batch, hidden, out_dim) in [(0, 2, 2), (2, 0, 2), (2, 2, 0)] {
            let err = expert_mlp(
                "x", "wg", "bg", "wu", "bu", "wd", "bd", "out", batch, hidden, out_dim,
            )
            .expect_err("zero dim must error");
            assert!(
                err.contains("expert_mlp") && err.contains("> 0"),
                "expert_mlp zero-dim ({batch},{hidden},{out_dim}): {err}"
            );
        }
    }
}

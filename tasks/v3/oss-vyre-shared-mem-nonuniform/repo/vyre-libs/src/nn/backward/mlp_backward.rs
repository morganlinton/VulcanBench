//! Backward for `mlp_4x_leaky_sq`:
//!
//! Forward: `out = W₂ · act(W₁ · x + b₁) + b₂` where act = leaky_relu_sq.
//!
//! Backward produces `grad_x`:
//! `grad_h_act = grad_out @ W₂^T` (hidden-dim grads after down-proj)
//! `grad_h = grad_h_act * d_act(h)` (through activation)
//! `grad_x = grad_h @ W₁^T` (back through up-proj)
//!
//! Simplified: computes grad_x by chaining transposes.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::nn::mlp_backward";

/// Backward for MLP (F32). Produces `grad_x[model_dim]`.
///
/// `x[model_dim]`, `w1[model_dim*hidden_dim]`, `b1[hidden_dim]`,
/// `w2[hidden_dim*model_dim]`, `grad_out[model_dim]`, `grad_x[model_dim]`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn mlp_backward(
    x: &str,
    w1: &str,
    b1: &str,
    w2: &str,
    grad_out: &str,
    grad_x: &str,
    model_dim: u32,
    hidden_dim: u32,
) -> Program {
    let i = Expr::var("i");

    // For each grad_x[i]:
    // grad_x[i] = sum_j (grad_h[j] * W1[i * hidden + j])
    // where grad_h[j] = d_act(h[j]) * sum_k (grad_out[k] * W2[j * model + k])
    // and h[j] = b1[j] + sum_m x[m] * W1[m * hidden + j]

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(model_dim)),
            vec![
                Node::let_bind("gx", Expr::f32(0.0)),
                Node::loop_for(
                    "j",
                    Expr::u32(0),
                    Expr::u32(hidden_dim),
                    vec![
                        // Compute h[j] = b1[j] + sum_m x[m]*W1[m*hidden+j]
                        Node::let_bind("h_acc", Expr::load(b1, Expr::var("j"))),
                        Node::loop_for(
                            "m",
                            Expr::u32(0),
                            Expr::u32(model_dim),
                            vec![Node::assign(
                                "h_acc",
                                Expr::add(
                                    Expr::var("h_acc"),
                                    Expr::mul(
                                        Expr::load(x, Expr::var("m")),
                                        Expr::load(
                                            w1,
                                            Expr::add(
                                                Expr::mul(Expr::var("m"), Expr::u32(hidden_dim)),
                                                Expr::var("j"),
                                            ),
                                        ),
                                    ),
                                ),
                            )],
                        ),
                        // d_act(h) = max(0.5*h, 2*h) (same branchless trick as backward op)
                        Node::let_bind(
                            "d_act",
                            Expr::max(
                                Expr::mul(Expr::f32(0.5), Expr::var("h_acc")),
                                Expr::mul(Expr::f32(2.0), Expr::var("h_acc")),
                            ),
                        ),
                        // grad_h_act[j] = sum_k grad_out[k] * W2[j*model+k]
                        Node::let_bind("gh_act", Expr::f32(0.0)),
                        Node::loop_for(
                            "k",
                            Expr::u32(0),
                            Expr::u32(model_dim),
                            vec![Node::assign(
                                "gh_act",
                                Expr::add(
                                    Expr::var("gh_act"),
                                    Expr::mul(
                                        Expr::load(grad_out, Expr::var("k")),
                                        Expr::load(
                                            w2,
                                            Expr::add(
                                                Expr::mul(Expr::var("j"), Expr::u32(model_dim)),
                                                Expr::var("k"),
                                            ),
                                        ),
                                    ),
                                ),
                            )],
                        ),
                        // grad_h[j] = gh_act * d_act
                        Node::let_bind("gh", Expr::mul(Expr::var("gh_act"), Expr::var("d_act"))),
                        // gx += gh * W1[i * hidden + j]
                        Node::assign(
                            "gx",
                            Expr::add(
                                Expr::var("gx"),
                                Expr::mul(
                                    Expr::var("gh"),
                                    Expr::load(
                                        w1,
                                        Expr::add(
                                            Expr::mul(i.clone(), Expr::u32(hidden_dim)),
                                            Expr::var("j"),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ],
                ),
                Node::Store {
                    buffer: grad_x.into(),
                    index: i,
                    value: Expr::var("gx"),
                },
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(model_dim),
            BufferDecl::storage(w1, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(model_dim * hidden_dim),
            BufferDecl::storage(b1, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim),
            BufferDecl::storage(w2, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim * model_dim),
            BufferDecl::storage(grad_out, 4, BufferAccess::ReadOnly, DataType::F32)
                .with_count(model_dim),
            BufferDecl::output(grad_x, 5, DataType::F32).with_count(model_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || mlp_backward("x", "w1", "b1", "w2", "grad_out", "grad_x", 2, 2),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_f32(&[1.0, 2.0]),           // x
                to_f32(&[1.0, 0.0, 0.0, 1.0]), // w1 = identity (2×2)
                to_f32(&[0.0, 0.0]),            // b1
                to_f32(&[1.0, 0.0, 0.0, 1.0]), // w2 = identity (2×2)
                to_f32(&[1.0, 1.0]),            // grad_out
                vec![0u8; 4 * 2],
            ]]
        }),
        expected_output: Some(|| {
            // W1=W2=I, b1=0, x=[1,2], grad_out=[1,1]
            // h = x = [1, 2], d_act = max(0.5*h, 2*h) = [2, 4]
            // grad_h_act[j] = sum_k grad_out[k]*W2[j*2+k] → W2=I so [1,1]
            // grad_h = d_act * grad_h_act = [2*1, 4*1] = [2, 4]
            // grad_x[i] = sum_j grad_h[j]*W1[i*2+j] → W1=I so [2, 4]
            let out = [2.0_f32, 4.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

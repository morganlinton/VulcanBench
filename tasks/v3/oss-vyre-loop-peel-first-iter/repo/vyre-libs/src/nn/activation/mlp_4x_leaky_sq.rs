//! MLP 4× LeakyReLU²: `y = W₂ · leaky_relu_sq(W₁ · x + b₁) + b₂`.
//!
//! Category A composition. Fused linear + activation without scratch buffer.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::{wrap_anonymous, wrap_child};
use vyre_foundation::ir::model::expr::GeneratorRef;

const OP_ID: &str = "vyre-libs::nn::mlp_4x_leaky_sq";
const MLP_WORKGROUP: u32 = 256;
const HIDDEN_SCRATCH: &str = "__mlp_4x_leaky_sq_hidden";
const HIDDEN_PROJECTION_OP_ID: &str = "vyre-libs::nn::mlp_4x_leaky_sq::hidden_projection";
const OUTPUT_PROJECTION_OP_ID: &str = "vyre-libs::nn::mlp_4x_leaky_sq::output_projection";

/// Build MLP with fused leaky_relu_sq activation (F32).
///
/// # Errors
/// Returns `Err` if any dimension is zero.
pub fn mlp_4x_leaky_sq(
    x: &str,
    w1: &str,
    b1: &str,
    w2: &str,
    b2: &str,
    output: &str,
    model_dim: u32,
    hidden_dim: u32,
) -> Result<Program, String> {
    if model_dim == 0 || hidden_dim == 0 {
        return Err("Fix: mlp requires non-zero dimensions".into());
    }
    let parent = GeneratorRef {
        name: OP_ID.to_string(),
    };
    let body = vec![
        Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
        wrap_child(
            HIDDEN_PROJECTION_OP_ID,
            parent.clone(),
            hidden_projection_body(x, w1, b1, model_dim, hidden_dim),
        ),
        Node::barrier(),
        wrap_child(
            OUTPUT_PROJECTION_OP_ID,
            parent,
            output_projection_body(w2, b2, output, model_dim, hidden_dim),
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(model_dim),
            BufferDecl::storage(w1, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(model_dim * hidden_dim),
            BufferDecl::storage(b1, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim),
            BufferDecl::storage(w2, 3, BufferAccess::ReadOnly, DataType::F32)
                .with_count(hidden_dim * model_dim),
            BufferDecl::storage(b2, 4, BufferAccess::ReadOnly, DataType::F32).with_count(model_dim),
            BufferDecl::output(output, 5, DataType::F32).with_count(model_dim),
            BufferDecl::workgroup(HIDDEN_SCRATCH, hidden_dim, DataType::F32),
        ],
        [MLP_WORKGROUP, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

fn hidden_projection_body(
    x: &str,
    w1: &str,
    b1: &str,
    model_dim: u32,
    hidden_dim: u32,
) -> Vec<Node> {
    vec![Node::loop_for(
        "hidden_chunk",
        Expr::u32(0),
        Expr::u32(hidden_dim.div_ceil(MLP_WORKGROUP)),
        vec![
            Node::let_bind(
                "j",
                Expr::add(
                    Expr::mul(Expr::var("hidden_chunk"), Expr::u32(MLP_WORKGROUP)),
                    Expr::var("lane"),
                ),
            ),
            Node::if_then(
                Expr::lt(Expr::var("j"), Expr::u32(hidden_dim)),
                vec![
                    Node::let_bind("h", Expr::load(b1, Expr::var("j"))),
                    Node::loop_for(
                        "k",
                        Expr::u32(0),
                        Expr::u32(model_dim),
                        vec![Node::assign(
                            "h",
                            Expr::add(
                                Expr::var("h"),
                                Expr::mul(
                                    Expr::load(x, Expr::var("k")),
                                    Expr::load(
                                        w1,
                                        Expr::add(
                                            Expr::mul(Expr::var("k"), Expr::u32(hidden_dim)),
                                            Expr::var("j"),
                                        ),
                                    ),
                                ),
                            ),
                        )],
                    ),
                    Node::let_bind(
                        "lk",
                        Expr::max(Expr::mul(Expr::f32(0.5), Expr::var("h")), Expr::var("h")),
                    ),
                    Node::store(
                        HIDDEN_SCRATCH,
                        Expr::var("j"),
                        Expr::mul(Expr::var("lk"), Expr::var("lk")),
                    ),
                ],
            ),
        ],
    )]
}

fn output_projection_body(
    w2: &str,
    b2: &str,
    output: &str,
    model_dim: u32,
    hidden_dim: u32,
) -> Vec<Node> {
    vec![Node::loop_for(
        "out_chunk",
        Expr::u32(0),
        Expr::u32(model_dim.div_ceil(MLP_WORKGROUP)),
        vec![
            Node::let_bind(
                "i",
                Expr::add(
                    Expr::mul(Expr::var("out_chunk"), Expr::u32(MLP_WORKGROUP)),
                    Expr::var("lane"),
                ),
            ),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(model_dim)),
                vec![
                    Node::let_bind("out_acc", Expr::load(b2, Expr::var("i"))),
                    Node::loop_for(
                        "j",
                        Expr::u32(0),
                        Expr::u32(hidden_dim),
                        vec![Node::assign(
                            "out_acc",
                            Expr::add(
                                Expr::var("out_acc"),
                                Expr::mul(
                                    Expr::load(HIDDEN_SCRATCH, Expr::var("j")),
                                    Expr::load(
                                        w2,
                                        Expr::add(
                                            Expr::mul(Expr::var("j"), Expr::u32(model_dim)),
                                            Expr::var("i"),
                                        ),
                                    ),
                                ),
                            ),
                        )],
                    ),
                    Node::store(output, Expr::var("i"), Expr::var("out_acc")),
                ],
            ),
        ],
    )]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            mlp_4x_leaky_sq("x", "w1", "b1", "w2", "b2", "out", 2, 4)
                .unwrap_or_else(|error| crate::invalid_program(OP_ID, format!("Fix: mlp_4x_leaky_sq fixture must build: {error}")))
        },
        test_inputs: Some(|| {
            let f = vyre_primitives::wire::pack_f32_slice;
            vec![vec![
                f(&[1.0, 2.0]),
                f(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]),
                f(&[0.0; 4]), f(&[1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0]),
                f(&[0.0, 0.0]),
            ]]
        }),
        expected_output: Some(|| {
            // model_dim=2, hidden_dim=4
            // x=[1,2], w1=[0.1,0.2,0.3,0.4, 0.5,0.6,0.7,0.8], b1=[0;4]
            // w2=[1,0,0,1, 1,0,0,1], b2=[0,0]
            let x = [1.0_f32, 2.0];
            let w1 = [0.1_f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
            let b1 = [0.0_f32; 4];
            let w2 = [1.0_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
            let b2 = [0.0_f32; 2];
            let model_dim = 2usize;
            let hidden_dim = 4usize;
            // h[j] = b1[j] + sum_k x[k]*w1[k*hid+j]
            let h: Vec<f32> = (0..hidden_dim).map(|j| {
                b1[j] + (0..model_dim).map(|k| x[k] * w1[k * hidden_dim + j]).sum::<f32>()
            }).collect();
            // act[j] = max(0.5*h, h)^2 = h^2 (all positive)
            let act: Vec<f32> = h.iter().map(|v| {
                let lk = v.max(0.5 * v);
                lk * lk
            }).collect();
            // out[i] = b2[i] + sum_j act[j]*w2[j*model+i]
            let out: Vec<f32> = (0..model_dim).map(|i| {
                b2[i] + (0..hidden_dim).map(|j| act[j] * w2[j * model_dim + i]).sum::<f32>()
            }).collect();
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

fn f32_fixture(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

fn hidden_projection_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(2),
            BufferDecl::storage("w1", 1, BufferAccess::ReadOnly, DataType::F32).with_count(8),
            BufferDecl::storage("b1", 2, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::output(HIDDEN_SCRATCH, 3, DataType::F32).with_count(4),
        ],
        [MLP_WORKGROUP, 1, 1],
        vec![wrap_anonymous(
            HIDDEN_PROJECTION_OP_ID,
            vec![
                Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
                Node::if_then(
                    Expr::lt(Expr::var("lane"), Expr::u32(MLP_WORKGROUP)),
                    hidden_projection_body("x", "w1", "b1", 2, 4),
                ),
            ],
        )],
    )
}

fn output_projection_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("w2", 0, BufferAccess::ReadOnly, DataType::F32).with_count(8),
            BufferDecl::storage("b2", 1, BufferAccess::ReadOnly, DataType::F32).with_count(2),
            BufferDecl::storage(HIDDEN_SCRATCH, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(4),
            BufferDecl::output("out", 3, DataType::F32).with_count(2),
        ],
        [MLP_WORKGROUP, 1, 1],
        vec![wrap_anonymous(
            OUTPUT_PROJECTION_OP_ID,
            vec![
                Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
                Node::if_then(
                    Expr::lt(Expr::var("lane"), Expr::u32(MLP_WORKGROUP)),
                    output_projection_body("w2", "b2", "out", 2, 4),
                ),
            ],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: HIDDEN_PROJECTION_OP_ID,
        build: hidden_projection_program,
        test_inputs: Some(|| vec![vec![
            f32_fixture(&[1.0, 2.0]),
            f32_fixture(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]),
            f32_fixture(&[0.0; 4]),
        ]]),
        expected_output: Some(|| {
            let x = [1.0_f32, 2.0];
            let w1 = [0.1_f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
            let mut out = [0.0_f32; 4];
            for j in 0..4 {
                let h = x[0] * w1[j] + x[1] * w1[4 + j];
                let lk = h.max(0.5 * h);
                out[j] = lk * lk;
            }
            vec![vec![f32_fixture(&out)]]
        }),
        category: Some("nn"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OUTPUT_PROJECTION_OP_ID,
        build: output_projection_program,
        test_inputs: Some(|| vec![vec![
            f32_fixture(&[1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0]),
            f32_fixture(&[0.0, 0.0]),
            f32_fixture(&[1.21, 1.96, 2.89, 4.0]),
        ]]),
        expected_output: Some(|| {
            let hidden = [1.21_f32, 1.96, 2.89, 4.0];
            let w2 = [1.0_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
            let mut out = [0.0_f32; 2];
            for i in 0..2 {
                for j in 0..4 {
                    out[i] += hidden[j] * w2[j * 2 + i];
                }
            }
            vec![vec![f32_fixture(&out)]]
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

    #[test]
    fn mlp_materializes_hidden_once_and_matches_reference() {
        let program = mlp_4x_leaky_sq("x", "w1", "b1", "w2", "b2", "out", 2, 4)
            .expect("Fix: fixture dimensions must build.");
        assert_eq!(program.workgroup_size(), [MLP_WORKGROUP, 1, 1]);
        let x = [1.0_f32, 2.0];
        let w1 = [0.1_f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let b1 = [0.0_f32; 4];
        let w2 = [1.0_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        let b2 = [0.0_f32, 0.0];
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&x)),
                Value::from(f32_bytes(&w1)),
                Value::from(f32_bytes(&b1)),
                Value::from(f32_bytes(&w2)),
                Value::from(f32_bytes(&b2)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: mlp_4x_leaky_sq must execute in the reference interpreter.");
        let actual = decode_f32(&outputs[0].to_bytes());
        let hidden = (0..4)
            .map(|j| {
                let h = b1[j] + (0..2).map(|k| x[k] * w1[k * 4 + j]).sum::<f32>();
                let lk = h.max(0.5 * h);
                lk * lk
            })
            .collect::<Vec<_>>();
        let expected = (0..2)
            .map(|i| b2[i] + (0..4).map(|j| hidden[j] * w2[j * 2 + i]).sum::<f32>())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}

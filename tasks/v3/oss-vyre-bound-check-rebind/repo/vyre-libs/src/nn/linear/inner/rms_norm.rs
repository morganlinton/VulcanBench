//! Fused `rms_norm_linear` constructor.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::nn::rms::{inverse_rms_expr, square_expr};
use crate::region::wrap_anonymous;
use crate::tensor_ref::TensorRefError;

/// Fused RMSNorm + linear: `out = (input / rms(input)) @ W + b`.
///
/// # Errors
/// Returns `Err` when dimensions are zero or buffer counts overflow `u32`.
pub fn rms_norm_linear(
    input: &str,
    w: &str,
    b: &str,
    out: &str,
    n: u32,
    in_dim: u32,
    out_dim: u32,
    eps: f32,
) -> Program {
    try_rms_norm_linear(input, w, b, out, n, in_dim, out_dim, eps).unwrap_or_else(|error| {
        crate::builder::invalid_output_program(
            "vyre-libs::nn::rms_norm_linear",
            out,
            DataType::F32,
            format!("Fix: rms_norm_linear build failed: {error}"),
        )
    })
}

/// Fallible fused RMSNorm + linear constructor.
///
/// # Errors
///
/// Returns [`TensorRefError`] when dimensions are incoherent or counts
/// overflow `u32`.
pub fn try_rms_norm_linear(
    input: &str,
    w: &str,
    b: &str,
    out: &str,
    n: u32,
    in_dim: u32,
    out_dim: u32,
    eps: f32,
) -> Result<Program, TensorRefError> {
    if n == 0 || in_dim == 0 || out_dim == 0 || n > in_dim {
        return Err(TensorRefError::ShapeMismatch {
            name: input.to_string(),
            found: vec![n, in_dim, out_dim],
            expected: vec![1, in_dim.max(1), out_dim.max(1)],
            op: "vyre-libs::nn::rms_norm_linear",
        });
    }
    let weight_count =
        in_dim
            .checked_mul(out_dim)
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: w.to_string(),
                shape: vec![in_dim, out_dim],
            })?;

    let lane = Expr::var("lane");
    let k = Expr::var("k");

    let mean_sq = vec![
        Node::let_bind("sum_sq", Expr::f32(0.0)),
        Node::loop_for(
            "k",
            Expr::u32(0),
            Expr::u32(n),
            vec![Node::assign(
                "sum_sq",
                Expr::add(
                    Expr::var("sum_sq"),
                    square_expr(Expr::load(input, k.clone())),
                ),
            )],
        ),
        Node::Store {
            buffer: "inv_rms".into(),
            index: Expr::u32(0),
            value: inverse_rms_expr(Expr::var("sum_sq"), n, eps),
        },
    ];

    let output_lane = vec![
        Node::let_bind("acc", Expr::load(b, lane.clone())),
        Node::let_bind("scale", Expr::load("inv_rms", Expr::u32(0))),
        Node::loop_for(
            "k",
            Expr::u32(0),
            Expr::u32(in_dim),
            vec![Node::assign(
                "acc",
                Expr::add(
                    Expr::var("acc"),
                    Expr::mul(
                        Expr::mul(Expr::load(input, k.clone()), Expr::var("scale")),
                        Expr::load(
                            w,
                            Expr::add(Expr::mul(k.clone(), Expr::u32(out_dim)), lane.clone()),
                        ),
                    ),
                ),
            )],
        ),
        Node::Store {
            buffer: out.into(),
            index: lane.clone(),
            value: Expr::var("acc"),
        },
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(weight_count),
            BufferDecl::storage(b, 2, BufferAccess::ReadOnly, DataType::F32).with_count(out_dim),
            BufferDecl::workgroup("inv_rms", 1, DataType::F32),
            BufferDecl::output(out, 4, DataType::F32).with_count(out_dim),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::nn::rms_norm_linear",
            vec![
                Node::let_bind("lane", Expr::InvocationId { axis: 0 }),
                Node::if_then(Expr::eq(lane.clone(), Expr::u32(0)), mean_sq),
                Node::barrier(),
                Node::if_then(Expr::lt(lane.clone(), Expr::u32(out_dim)), output_lane),
            ],
        )],
    )
    .with_entry_op_id("vyre-libs::nn::rms_norm_linear"))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::rms_norm_linear",
        build: || rms_norm_linear("input", "w", "b", "out", 4, 4, 4, 1e-5),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            let input = [1.0_f32, 2.0, 3.0, 4.0];
            let weights = (0u32..16u32).map(|v| v as f32).collect::<Vec<_>>();
            vec![vec![
                to_bytes(&input),
                to_bytes(&weights),
                vec![0u8; 4 * 4],
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_f32_slice;
            let input = [1.0_f32, 2.0, 3.0, 4.0];
            let eps = 1e-5_f32;
            let inv_scale =
                1.0_f32 / (input.iter().map(|v| v * v).sum::<f32>() / 4.0_f32 + eps).sqrt();
            let mut out = Vec::with_capacity(4);
            for j in 0..4usize {
                let mut acc = 0.0_f32;
                for k in 0..4usize {
                    acc += input[k] * inv_scale * (k * 4 + j) as f32;
                }
                out.push(acc);
            }
            vec![vec![to_bytes(&out)]]
        }),
        category: Some("nn"),
    }
}

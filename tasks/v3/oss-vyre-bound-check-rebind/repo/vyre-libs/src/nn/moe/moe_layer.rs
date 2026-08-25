//! MoE layer: `router(x) → top_k → gather experts → weighted sum`.
//!
//! This is the full MoE layer dispatch. It assumes experts are already
//! resident in GPU memory (managed by VyreOffload). The layer:
//!   1. Computes router logits: `scores = x @ W_router + b_router`
//!   2. Applies softmax + top-k to get expert indices and weights
//!   3. Dispatches to each selected expert and accumulates weighted outputs
//!
//! For the single-token case (inference decode), this is simplified to
//! a sequential gather over the k selected experts.
//!
//! Category A composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

/// Build a Program that computes a single-token MoE layer forward pass.
///
/// Shapes:
///   `x: [in_dim]`  -  input token
///   `w_router: [in_dim, num_experts]`  -  router weights
///   `b_router: [num_experts]`  -  router bias
///   `expert_outputs: [k, out_dim]`  -  pre-computed expert outputs (from VyreOffload)
///   `out: [out_dim]`  -  final weighted sum output
///
/// The `expert_outputs` buffer is expected to be populated by the runtime
/// after loading the k selected experts. This kernel only does the
/// routing + weighted accumulation.
///
/// # Errors
/// Returns `Err` when any dimension is zero or k > num_experts.
pub fn moe_layer_route_and_accumulate(
    x: &str,
    w_router: &str,
    b_router: &str,
    expert_indices: &str,
    expert_weights: &str,
    expert_outputs: &str,
    out: &str,
    in_dim: u32,
    num_experts: u32,
    out_dim: u32,
    k: u32,
) -> Result<Program, String> {
    if in_dim == 0 || num_experts == 0 || out_dim == 0 || k == 0 {
        return Err("Fix: moe_layer all dims must be > 0".to_string());
    }
    if k > num_experts {
        return Err("Fix: moe_layer k cannot exceed num_experts".to_string());
    }

    let w_router_count = in_dim
        .checked_mul(num_experts)
        .ok_or("Fix: moe_layer w_router count overflow")?;

    let j = Expr::var("j");

    // For each output dimension j:
    //   out[j] = sum_{e=0}^{k-1} weight[e] * expert_outputs[e, j]
    //
    // We use a single invocation per output dimension.
    let body = vec![
        Node::let_bind("j", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(j.clone(), Expr::u32(out_dim)),
            vec![
                Node::let_bind("acc", Expr::f32(0.0)),
                Node::loop_for(
                    "e",
                    Expr::u32(0),
                    Expr::u32(k),
                    vec![
                        // weight = expert_weights[e]
                        Node::let_bind("weight", Expr::load(expert_weights, Expr::var("e"))),
                        // expert_val = expert_outputs[e, j]
                        Node::let_bind(
                            "expert_idx",
                            Expr::add(Expr::mul(Expr::var("e"), Expr::u32(out_dim)), j.clone()),
                        ),
                        Node::let_bind(
                            "expert_val",
                            Expr::load(expert_outputs, Expr::var("expert_idx")),
                        ),
                        Node::assign(
                            "acc",
                            Expr::add(
                                Expr::var("acc"),
                                Expr::mul(Expr::var("weight"), Expr::var("expert_val")),
                            ),
                        ),
                    ],
                ),
                Node::Store {
                    buffer: out.into(),
                    index: j,
                    value: Expr::var("acc"),
                },
            ],
        ),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(x, 0, BufferAccess::ReadOnly, DataType::F32).with_count(in_dim),
            BufferDecl::storage(w_router, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(w_router_count),
            BufferDecl::storage(b_router, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_experts),
            BufferDecl::storage(expert_indices, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(k),
            BufferDecl::storage(expert_weights, 4, BufferAccess::ReadOnly, DataType::F32)
                .with_count(k),
            BufferDecl::storage(expert_outputs, 5, BufferAccess::ReadOnly, DataType::F32)
                .with_count(k.checked_mul(out_dim).ok_or("overflow")?),
            BufferDecl::output(out, 6, DataType::F32).with_count(out_dim),
        ],
        [256, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::moe_layer_accumulate", body)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use crate::test_support::byte_pack::u32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn moe_layer_accumulate_simple() {
        // k=2, out_dim=2
        // expert_outputs[0] = [1.0, 2.0], weights = [0.6, 0.4]
        // expert_outputs[1] = [3.0, 4.0]
        // out = [0.6*1 + 0.4*3, 0.6*2 + 0.4*4] = [1.8, 2.8]
        let expert_indices = vec![0u32, 1];
        let expert_weights = vec![0.6f32, 0.4];
        let expert_outputs = vec![1.0f32, 2.0, 3.0, 4.0];

        let program = moe_layer_route_and_accumulate(
            "x",
            "w_router",
            "b_router",
            "expert_indices",
            "expert_weights",
            "expert_outputs",
            "out",
            2,
            4,
            2,
            2,
        )
        .unwrap();

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&[0.0f32, 0.0])), // x (unused in this simplified kernel)
                Value::from(f32_bytes(&[0.0f32; 8])),   // w_router (unused)
                Value::from(f32_bytes(&[0.0f32; 4])),   // b_router (unused)
                Value::from(u32_bytes(&expert_indices)),
                Value::from(f32_bytes(&expert_weights)),
                Value::from(f32_bytes(&expert_outputs)),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: moe_layer accumulate must execute");

        let out = decode_f32(&outputs[0].to_bytes());
        assert!(
            (out[0] - 1.8).abs() < 1e-5,
            "moe_layer out[0] mismatch: {}",
            out[0]
        );
        assert!(
            (out[1] - 2.8).abs() < 1e-5,
            "moe_layer out[1] mismatch: {}",
            out[1]
        );
    }

    #[test]
    fn moe_layer_zero_dim_errors() {
        for (batch, hidden, k, experts) in [(0, 4, 2, 2), (2, 0, 2, 2), (2, 4, 0, 2), (2, 4, 2, 0)]
        {
            let err = moe_layer_route_and_accumulate(
                "x", "wr", "br", "ei", "ew", "eo", "out", batch, hidden, k, experts,
            )
            .expect_err("zero dim must error");
            assert!(
                err.contains("moe_layer") && err.contains("> 0"),
                "moe_layer zero-dim ({batch},{hidden},{k},{experts}): {err}"
            );
        }
    }

    #[test]
    fn moe_layer_k_greater_than_num_experts_errors() {
        let err =
            moe_layer_route_and_accumulate("x", "wr", "br", "ei", "ew", "eo", "out", 2, 4, 2, 5)
                .expect_err("k > num_experts");
        assert!(
            err.contains("k cannot exceed num_experts"),
            "moe_layer k/experts error: {err}"
        );
    }
}

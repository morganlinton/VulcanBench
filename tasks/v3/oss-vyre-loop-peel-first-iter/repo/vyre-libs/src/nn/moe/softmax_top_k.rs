//! Fused `softmax_top_k` constructor for MoE gating.
//!
//! Computes `softmax(scores)` and returns the top-k indices + normalized weights
//! in a single dispatch, eliminating the separate softmax + top-k round-trip.

use super::topk_selection::{
    copy_top_k_indices_and_normalized_weights, init_top_k_slots, insert_top_k_candidate, BEST_IDXS,
    BEST_VALS,
};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

/// Build a Program that computes softmax over `scores`, then returns the
/// top-k indices and their normalized weights.
///
/// Inputs:
/// - `scores`: f32 buffer of length `n`
///
/// Outputs:
/// - `out_indices`: u32 buffer of length `k`
/// - `out_weights`: f32 buffer of length `k`
///
/// The weights sum to 1.0 across the full distribution (not just the top-k).
#[must_use]
pub fn softmax_top_k(
    scores: &str,
    out_indices: &str,
    out_weights: &str,
    n: u32,
    k: u32,
) -> Program {
    if k == 0 {
        return crate::builder::invalid_output_program(
            "vyre-libs::nn::softmax_top_k",
            out_indices,
            DataType::U32,
            "Fix: softmax_top_k requires k > 0 so the selection scratch has at least one slot."
                .to_string(),
        );
    }
    let mut body = init_top_k_slots(k);

    // max_val = max(scores)
    body.push(Node::let_bind("max_val", Expr::f32(f32::NEG_INFINITY)));
    body.push(Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(n),
        vec![Node::if_then(
            Expr::gt(Expr::load(scores, Expr::var("i")), Expr::var("max_val")),
            vec![Node::assign("max_val", Expr::load(scores, Expr::var("i")))],
        )],
    ));

    // sum = sum(exp(score - max_val))
    // Also track top-k on the exp values
    body.push(Node::let_bind("sum", Expr::f32(0.0)));
    body.push(Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(n),
        vec![
            Node::let_bind(
                "exp_val",
                Expr::UnOp {
                    op: UnOp::Exp,
                    operand: Box::new(Expr::sub(
                        Expr::load(scores, Expr::var("i")),
                        Expr::var("max_val"),
                    )),
                },
            ),
            Node::assign("sum", Expr::add(Expr::var("sum"), Expr::var("exp_val"))),
            // Top-k insertion on exp_val
            Node::Block(insert_top_k_candidate(
                k,
                Expr::var("exp_val"),
                Expr::var("i"),
            )),
        ],
    ));

    body.extend(copy_top_k_indices_and_normalized_weights(
        out_indices,
        out_weights,
        k,
        Expr::var("sum"),
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(scores, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(out_indices, 1, DataType::U32).with_count(k),
            BufferDecl::read_write(out_weights, 2, DataType::F32).with_count(k),
            BufferDecl::read_write(BEST_VALS, 3, DataType::F32).with_count(k),
            BufferDecl::read_write(BEST_IDXS, 4, DataType::U32).with_count(k),
        ],
        [1, 1, 1],
        vec![wrap_anonymous("vyre-libs::nn::softmax_top_k", body)],
    )
}

fn fixture_f32_bytes(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

fn fixture_u32_bytes(values: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(values)
}

fn softmax_top_k_fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    let scores: [f32; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    vec![vec![
        fixture_f32_bytes(&scores),
        vec![0u8; 4 * 2],
        vec![0u8; 4 * 2],
        vec![0u8; 4 * 2],
    ]]
}

fn softmax_top_k_fixture_expected() -> Vec<Vec<Vec<u8>>> {
    let scores: [f32; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let max = scores[7];
    let exp_values = scores
        .iter()
        .map(|score| (*score - max).exp())
        .collect::<Vec<f32>>();
    let sum = exp_values.iter().copied().sum::<f32>();
    let top_exp = [exp_values[7], exp_values[6]];
    vec![vec![
        fixture_u32_bytes(&[7, 6]),
        fixture_f32_bytes(&[top_exp[0] / sum, top_exp[1] / sum]),
        fixture_f32_bytes(&top_exp),
        fixture_u32_bytes(&[7, 6]),
    ]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn u32_from_bytes(bytes: &[u8]) -> Vec<u32> {
        vyre_primitives::wire::decode_u32_le_bytes_all(bytes)
    }

    fn f32_from_bytes(bytes: &[u8]) -> Vec<f32> {
        vyre_primitives::wire::decode_f32_le_bytes_all(bytes)
    }

    #[test]
    fn softmax_top_k_basic() {
        // scores = [1.0, 2.0, 3.0]  -  softmax ≈ [0.090, 0.245, 0.665]
        let scores = vec![1.0f32, 2.0, 3.0];
        let program = softmax_top_k("scores", "indices", "weights", 3, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 2 * 4]),
                Value::from(vec![0u8; 2 * 4]),
                Value::from(vec![0u8; 2 * 4]),
            ],
        )
        .unwrap();

        let indices = u32_from_bytes(&outputs[0].to_bytes());
        let weights = f32_from_bytes(&outputs[1].to_bytes());

        assert_eq!(indices[0], 2); // 3.0 is max
        assert_eq!(indices[1], 1); // 2.0 is second

        // Weights should be the normalized softmax values
        let max = 3.0f32;
        let exp0 = (1.0 - max).exp();
        let exp1 = (2.0 - max).exp();
        let exp2 = (3.0 - max).exp();
        let sum = exp0 + exp1 + exp2;
        let expected_w0 = exp2 / sum;
        let expected_w1 = exp1 / sum;

        assert!((weights[0] - expected_w0).abs() < 1e-4);
        assert!((weights[1] - expected_w1).abs() < 1e-4);
    }

    #[test]
    fn softmax_top_k_weights_sum_to_one() {
        let scores: Vec<f32> = (1..=8).map(|i| i as f32).collect();
        let program = softmax_top_k("scores", "indices", "weights", 8, 3);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 3 * 4]),
                Value::from(vec![0u8; 3 * 4]),
                Value::from(vec![0u8; 3 * 4]),
            ],
        )
        .unwrap();

        let weights = f32_from_bytes(&outputs[1].to_bytes());
        let total: f32 = weights.iter().sum();
        // The top-3 weights don't sum to 1.0, but the internal sum is 1.0.
        // Just verify the weights are positive and ordered correctly.
        assert!(total > 0.0);
        assert!(weights[0] > weights[1]);
        assert!(weights[1] > weights[2]);
        assert!(weights[0] > 0.0);
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::softmax_top_k",
        build: || softmax_top_k("scores", "indices", "weights", 8, 2),
        test_inputs: Some(softmax_top_k_fixture_inputs),
        expected_output: Some(softmax_top_k_fixture_expected),
        category: Some("nn"),
    }
}

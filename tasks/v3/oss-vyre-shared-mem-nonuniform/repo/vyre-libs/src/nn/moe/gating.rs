//! MoE Gating: softmax(scores) + top-k selection.
//!
//! Category-A composition over `nn::softmax` and `nn::top_k`.

use crate::region::{wrap_anonymous, wrap_child};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_primitives::nn::quest_paging_passes::{quest_select_top_k_body, QUEST_SELECT_TOP_K_OP_ID};

const OP_ID: &str = "vyre-libs::nn::moe_gate";
const SCORES_SCRATCH: &str = "__moe_gate_scores_scratch";
const STATS_SCRATCH: &str = "__moe_gate_stats";
const SOFTMAX_STATS_OP_ID: &str = "vyre-libs::nn::moe_gate::softmax_stats";
const WEIGHT_WRITE_OP_ID: &str = "vyre-libs::nn::moe_gate::weight_write";

/// Build a Program that computes MoE gating.
/// `input_scores`: `num_experts`, `output_indices`: `k`, `output_weights`: `k`.
#[must_use]
pub fn moe_gate(
    input_scores: &str,
    output_indices: &str,
    output_weights: &str,
    num_experts: u32,
    k: u32,
) -> Program {
    // Lane-0 deterministic gate: stable softmax denominator followed
    // by duplicate-suppressed top-k selection.
    let parent = GeneratorRef {
        name: OP_ID.to_string(),
    };
    let body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            wrap_child(
                SOFTMAX_STATS_OP_ID,
                parent.clone(),
                softmax_stats_body(input_scores, num_experts),
            ),
            wrap_child(
                QUEST_SELECT_TOP_K_OP_ID,
                parent.clone(),
                quest_select_top_k_body(SCORES_SCRATCH, output_indices, num_experts, k, f32::MIN),
            ),
            wrap_child(
                WEIGHT_WRITE_OP_ID,
                parent,
                weight_write_body(input_scores, output_indices, output_weights, k),
            ),
        ],
    )];

    // V022: a Program may declare at most one ::output buffer.
    // `output_weights` is the scalar gating result the reference
    // interpreter compares against; `output_indices` is a read-write
    // storage buffer the caller consumes alongside.
    Program::wrapped(
        vec![
            BufferDecl::storage(input_scores, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_experts),
            BufferDecl::storage(output_indices, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(k),
            BufferDecl::output(output_weights, 2, DataType::F32).with_count(k),
            BufferDecl::workgroup(SCORES_SCRATCH, num_experts, DataType::F32),
            BufferDecl::workgroup(STATS_SCRATCH, 2, DataType::F32),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

fn softmax_stats_body(input_scores: &str, num_experts: u32) -> Vec<Node> {
    vec![
        Node::let_bind("max_score", Expr::f32(f32::MIN)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(num_experts),
            vec![
                Node::let_bind("score", Expr::load(input_scores, Expr::var("i"))),
                Node::store(SCORES_SCRATCH, Expr::var("i"), Expr::var("score")),
                Node::assign(
                    "max_score",
                    Expr::max(Expr::var("max_score"), Expr::var("score")),
                ),
            ],
        ),
        Node::let_bind("sum_exp", Expr::f32(0.0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(num_experts),
            vec![Node::assign(
                "sum_exp",
                Expr::add(
                    Expr::var("sum_exp"),
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(Expr::sub(
                            Expr::load(input_scores, Expr::var("i")),
                            Expr::var("max_score"),
                        )),
                    },
                ),
            )],
        ),
        Node::store(STATS_SCRATCH, Expr::u32(0), Expr::var("max_score")),
        Node::store(STATS_SCRATCH, Expr::u32(1), Expr::var("sum_exp")),
    ]
}

fn weight_write_body(
    input_scores: &str,
    output_indices: &str,
    output_weights: &str,
    k: u32,
) -> Vec<Node> {
    vec![
        Node::let_bind("max_score", Expr::load(STATS_SCRATCH, Expr::u32(0))),
        Node::let_bind("sum_exp", Expr::load(STATS_SCRATCH, Expr::u32(1))),
        Node::loop_for(
            "j",
            Expr::u32(0),
            Expr::u32(k),
            vec![
                Node::let_bind("best_idx", Expr::load(output_indices, Expr::var("j"))),
                Node::let_bind(
                    "best_score",
                    Expr::load(input_scores, Expr::var("best_idx")),
                ),
                Node::store(
                    output_weights,
                    Expr::var("j"),
                    Expr::div(
                        Expr::UnOp {
                            op: UnOp::Exp,
                            operand: Box::new(Expr::sub(
                                Expr::var("best_score"),
                                Expr::var("max_score"),
                            )),
                        },
                        Expr::var("sum_exp"),
                    ),
                ),
            ],
        ),
    ]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || moe_gate("scores", "indices", "weights", 8, 2),
        // Buffer order: scores (read-only f32 × 8), indices
        // (read-write u32 × 2), weights (output f32 × 2).
        test_inputs: Some(|| {
            let scores: [f32; 8] = [0.5, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            let scores_bytes = vyre_primitives::wire::pack_f32_slice(&scores);
            vec![vec![scores_bytes, vec![0u8; 4 * 2], vec![0u8; 4 * 2]]]
        }),
        expected_output: Some(|| {
            let scores: [f32; 8] = [0.5, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp = scores
                .iter()
                .map(|score| libm::expf(*score - max_score))
                .sum::<f32>();
            let indices: [u32; 2] = [5, 3];
            let idx_bytes = vyre_primitives::wire::pack_u32_slice(&indices);
            let expected_weights = [
                libm::expf(scores[5] - max_score) / sum_exp,
                libm::expf(scores[3] - max_score) / sum_exp,
            ];
            let weights = vyre_primitives::wire::pack_f32_slice(&expected_weights);
            vec![vec![idx_bytes, weights]]
        }),
        category: Some("nn"),
    }
}

fn f32_fixture(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

fn u32_fixture(values: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(values)
}

fn softmax_stats_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("scores", 0, BufferAccess::ReadOnly, DataType::F32).with_count(8),
            BufferDecl::storage(SCORES_SCRATCH, 1, BufferAccess::ReadWrite, DataType::F32)
                .with_count(8),
            BufferDecl::storage(STATS_SCRATCH, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(2),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            SOFTMAX_STATS_OP_ID,
            vec![Node::if_then(
                Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                softmax_stats_body("scores", 8),
            )],
        )],
    )
}

fn weight_write_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("scores", 0, BufferAccess::ReadOnly, DataType::F32).with_count(8),
            BufferDecl::storage("indices", 1, BufferAccess::ReadOnly, DataType::U32).with_count(2),
            BufferDecl::storage(STATS_SCRATCH, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(2),
            BufferDecl::output("weights", 3, DataType::F32).with_count(2),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            WEIGHT_WRITE_OP_ID,
            vec![Node::if_then(
                Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                weight_write_body("scores", "indices", "weights", 2),
            )],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SOFTMAX_STATS_OP_ID,
        build: softmax_stats_program,
        test_inputs: Some(|| {
            let scores = [0.5_f32, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            vec![vec![
                f32_fixture(&scores),
                f32_fixture(&[0.0; 8]),
                f32_fixture(&[0.0; 2]),
            ]]
        }),
        expected_output: Some(|| {
            let scores = [0.5_f32, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp = scores
                .iter()
                .map(|score| libm::expf(*score - max_score))
                .sum::<f32>();
            vec![vec![f32_fixture(&scores), f32_fixture(&[max_score, sum_exp])]]
        }),
        category: Some("nn"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: WEIGHT_WRITE_OP_ID,
        build: weight_write_program,
        test_inputs: Some(|| {
            let scores = [0.5_f32, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp = scores
                .iter()
                .map(|score| libm::expf(*score - max_score))
                .sum::<f32>();
            vec![vec![
                f32_fixture(&scores),
                u32_fixture(&[5, 3]),
                f32_fixture(&[max_score, sum_exp]),
                f32_fixture(&[0.0; 2]),
            ]]
        }),
        expected_output: Some(|| {
            let scores = [0.5_f32, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
            let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum_exp = scores
                .iter()
                .map(|score| libm::expf(*score - max_score))
                .sum::<f32>();
            vec![vec![f32_fixture(&[
                libm::expf(scores[5] - max_score) / sum_exp,
                libm::expf(scores[3] - max_score) / sum_exp,
            ])]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn u32_words(bytes: &[u8]) -> Vec<u32> {
        vyre_primitives::wire::decode_u32_le_bytes_all(bytes)
    }

    fn f32_words(bytes: &[u8]) -> Vec<f32> {
        vyre_primitives::wire::decode_f32_le_bytes_all(bytes)
    }

    #[test]
    fn moe_gate_outputs_unique_top_k_softmax_weights() {
        let scores: [f32; 8] = [0.5, 1.0, 0.1, 2.0, 0.3, 3.0, 0.2, 0.4];
        let program = moe_gate("scores", "indices", "weights", 8, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&scores)),
                Value::from(vec![0u8; 8]),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: moe_gate must execute in the reference interpreter.");

        assert_eq!(u32_words(&outputs[0].to_bytes()), vec![5, 3]);
        let weights = f32_words(&outputs[1].to_bytes());
        let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let sum_exp = scores
            .iter()
            .map(|score| libm::expf(*score - max_score))
            .sum::<f32>();
        let expected = [
            libm::expf(scores[5] - max_score) / sum_exp,
            libm::expf(scores[3] - max_score) / sum_exp,
        ];
        for (actual, expected) in weights.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() <= 1.0e-6);
        }
    }
}

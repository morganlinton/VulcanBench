//! `compound.pipeline.fused_filter.1m` - fused literal/dataflow/score filtering.
//!
//! This workload is intentionally compound: each lane performs a literal-style
//! hash predicate, a dataflow liveness check, a score threshold, and a
//! taint-class compatibility check before writing one compact candidate score.
//! The point is to measure one resident GPU program that would otherwise be a
//! chain of CPU-side passes with intermediate materialization.

use super::byte_pack::u32_bytes;
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use rayon::prelude::*;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const ITEM_COUNT: u32 = 1 << 20;
const HASH_SALT: u32 = 2_654_435_761;
const SCORE_BASE: u32 = 500;
const SCORE_SPAN_MASK: u32 = 127;

const SUITES: &[SuiteKind] = &[
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

pub struct CompoundFusedFilter;

struct CompoundFusedFilterPrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    accepted_count: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for CompoundFusedFilter {
    fn id(&self) -> BenchId {
        BenchId("compound.pipeline.fused_filter.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Compound Fused Filter 1M".to_string(),
            description:
                "One resident GPU pass fusing literal hash, dataflow liveness, score threshold, and taint-class filtering"
                    .to_string(),
            tags: vec![
                "compound".to_string(),
                "resident".to_string(),
                "dataflow".to_string(),
                "matching".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Runtime,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(u64::from(ITEM_COUNT) * 16),
            min_input_bytes: Some(u64::from(ITEM_COUNT) * 12),
            feature_set: vec!["compound.pipeline".to_string(), "resident".to_string()],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "fused compound rule/dataflow filtering",
            "rayon",
            "Rayon-parallel staged CPU filter with equivalent predicates",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = compound_program();
        let (tokens, scores, states) = compound_inputs();
        let inputs = vec![u32_bytes(&tokens), u32_bytes(&scores), u32_bytes(&states)];
        let input_bytes_total = input_bytes_total(&inputs);
        let baseline_start = std::time::Instant::now();
        let baseline_words = compound_cpu_oracle_checked(&tokens, &scores, &states)?;
        let baseline_wall_ns =
            u64::try_from(baseline_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let accepted_count = baseline_words.iter().filter(|&&value| value != 0).count() as u64;
        let baseline_output = u32_bytes(&baseline_words);
        let resident = ResidentInputSet::upload_with_zeroed_outputs_optional(
            ctx,
            &inputs,
            &[baseline_output.len()],
            "compound fused filter",
        )?;

        Ok(Box::new(CompoundFusedFilterPrepared {
            program,
            inputs,
            input_bytes_total,
            baseline_output,
            baseline_wall_ns,
            accepted_count,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<CompoundFusedFilterPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<CompoundFusedFilterPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "compound fused filter prepared payload type mismatch".to_string(),
                )
            })?;
        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let timed = dispatch.timed;
        let outputs = timed.outputs;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes,
            dispatch.resident_used,
        );

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: vec![
                    MetricPoint {
                        name: "compound_items".to_string(),
                        value: u64::from(ITEM_COUNT),
                    },
                    MetricPoint {
                        name: "compound_fused_predicates".to_string(),
                        value: 4,
                    },
                    MetricPoint {
                        name: "compound_cpu_passes_elided".to_string(),
                        value: 3,
                    },
                    MetricPoint {
                        name: "compound_accepted_items".to_string(),
                        value: prepared.accepted_count,
                    },
                    MetricPoint {
                        name: "resident_buffers".to_string(),
                        value: u64::from(dispatch.resident_used),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                bytes_touched: Some(
                    prepared
                        .input_bytes_total
                        .saturating_add(prepared.baseline_output.len() as u64),
                ),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<CompoundFusedFilterPrepared>()
            .map(|prepared| (prepared.input_bytes_total, u64::from(ITEM_COUNT) * 4))
            .unwrap_or((u64::from(ITEM_COUNT) * 12, u64::from(ITEM_COUNT) * 4))
    }
}

fn compound_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("tokens", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::storage("scores", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::storage("states", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::output("accepted", 3, DataType::U32).with_count(ITEM_COUNT),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("tid", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("tid"), Expr::u32(ITEM_COUNT)),
                vec![
                    Node::let_bind("token", Expr::load("tokens", Expr::var("tid"))),
                    Node::let_bind("score", Expr::load("scores", Expr::var("tid"))),
                    Node::let_bind("state", Expr::load("states", Expr::var("tid"))),
                    Node::let_bind(
                        "mixed",
                        Expr::bitxor(
                            Expr::var("token"),
                            Expr::mul(Expr::var("state"), Expr::u32(HASH_SALT)),
                        ),
                    ),
                    Node::let_bind(
                        "score_floor",
                        Expr::add(
                            Expr::u32(SCORE_BASE),
                            Expr::bitand(
                                Expr::shr(Expr::var("state"), Expr::u32(8)),
                                Expr::u32(SCORE_SPAN_MASK),
                            ),
                        ),
                    ),
                    Node::let_bind(
                        "literal_hit",
                        Expr::eq(
                            Expr::bitand(Expr::var("mixed"), Expr::u32(0x1f)),
                            Expr::u32(0),
                        ),
                    ),
                    Node::let_bind(
                        "dataflow_live",
                        Expr::ne(Expr::bitand(Expr::var("state"), Expr::u32(1)), Expr::u32(0)),
                    ),
                    Node::let_bind(
                        "score_ok",
                        Expr::ge(Expr::var("score"), Expr::var("score_floor")),
                    ),
                    Node::let_bind(
                        "taint_class_ok",
                        Expr::eq(
                            Expr::bitand(Expr::shr(Expr::var("mixed"), Expr::u32(5)), Expr::u32(3)),
                            Expr::bitand(Expr::var("state"), Expr::u32(3)),
                        ),
                    ),
                    Node::let_bind(
                        "accepted_predicate",
                        Expr::and(
                            Expr::and(Expr::var("literal_hit"), Expr::var("dataflow_live")),
                            Expr::and(Expr::var("score_ok"), Expr::var("taint_class_ok")),
                        ),
                    ),
                    Node::store(
                        "accepted",
                        Expr::var("tid"),
                        Expr::select(
                            Expr::var("accepted_predicate"),
                            Expr::add(
                                Expr::bitand(Expr::var("mixed"), Expr::u32(0xffff)),
                                Expr::var("score"),
                            ),
                            Expr::u32(0),
                        ),
                    ),
                ],
            ),
        ],
    )
}

fn compound_inputs() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut tokens = Vec::with_capacity(ITEM_COUNT as usize);
    let mut scores = Vec::with_capacity(ITEM_COUNT as usize);
    let mut states = Vec::with_capacity(ITEM_COUNT as usize);
    for index in 0..ITEM_COUNT {
        let token = mix32(index ^ 0xA5A5_5A5A);
        let state = mix32(index.wrapping_mul(17).wrapping_add(0x9E37_79B9)) | 1;
        let score = 440 + (mix32(index ^ 0x517C_C1B7) & 255);
        tokens.push(token);
        scores.push(score);
        states.push(state);
    }
    (tokens, scores, states)
}

fn compound_cpu_oracle(tokens: &[u32], scores: &[u32], states: &[u32]) -> Vec<u32> {
    tokens
        .par_iter()
        .zip(scores.par_iter())
        .zip(states.par_iter())
        .map(|((&token, &score), &state)| compound_acceptance_value(token, score, state))
        .collect()
}

fn compound_cpu_oracle_checked(
    tokens: &[u32],
    scores: &[u32],
    states: &[u32],
) -> Result<Vec<u32>, BenchError> {
    if tokens.len() != scores.len() || tokens.len() != states.len() {
        return Err(BenchError::ExecutionFailed(format!(
            "compound fused filter input length mismatch: tokens={}, scores={}, states={}. Fix: generate equal-length streams before building the CPU oracle.",
            tokens.len(),
            scores.len(),
            states.len()
        )));
    }
    Ok(compound_cpu_oracle(tokens, scores, states))
}

fn compound_acceptance_value(token: u32, score: u32, state: u32) -> u32 {
    let mixed = token ^ state.wrapping_mul(HASH_SALT);
    let score_floor = SCORE_BASE + ((state >> 8) & SCORE_SPAN_MASK);
    let literal_hit = mixed & 0x1f == 0;
    let dataflow_live = state & 1 != 0;
    let score_ok = score >= score_floor;
    let taint_class_ok = ((mixed >> 5) & 3) == (state & 3);
    if literal_hit && dataflow_live && score_ok && taint_class_ok {
        (mixed & 0xffff).wrapping_add(score)
    } else {
        0
    }
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

inventory::submit! {
    &CompoundFusedFilter as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_acceptance_vectors_cover_thousands_of_state_classes() {
        for index in 0..8192_u32 {
            let state = mix32(index).wrapping_shl(1) | 1;
            let mixed = (index << 7) | ((state & 3) << 5);
            let token = mixed ^ state.wrapping_mul(HASH_SALT);
            let score = SCORE_BASE + ((state >> 8) & SCORE_SPAN_MASK) + (index & 7);
            let accepted = compound_acceptance_value(token, score, state);

            assert_eq!(accepted, (mixed & 0xffff).wrapping_add(score));
        }
    }

    #[test]
    fn generated_rejection_vectors_cover_each_fused_predicate() {
        for index in 0..4096_u32 {
            let live_state = mix32(index).wrapping_shl(1) | 1;
            let accepting_mixed = (index << 7) | ((live_state & 3) << 5);
            let accepting_token = accepting_mixed ^ live_state.wrapping_mul(HASH_SALT);
            let accepting_score = SCORE_BASE + ((live_state >> 8) & SCORE_SPAN_MASK);

            let literal_miss_token = (accepting_mixed | 1) ^ live_state.wrapping_mul(HASH_SALT);
            assert_eq!(
                compound_acceptance_value(literal_miss_token, accepting_score, live_state),
                0
            );

            let dead_state = live_state & !1;
            let dead_mixed = (index << 7) | ((dead_state & 3) << 5);
            let dead_token = dead_mixed ^ dead_state.wrapping_mul(HASH_SALT);
            let dead_score = SCORE_BASE + ((dead_state >> 8) & SCORE_SPAN_MASK);
            assert_eq!(
                compound_acceptance_value(dead_token, dead_score, dead_state),
                0
            );

            let low_score = accepting_score.saturating_sub(1);
            assert_eq!(
                compound_acceptance_value(accepting_token, low_score, live_state),
                0
            );

            let wrong_class = (((live_state & 3) + 1) & 3) << 5;
            let class_miss_mixed = (index << 7) | wrong_class;
            let class_miss_token = class_miss_mixed ^ live_state.wrapping_mul(HASH_SALT);
            assert_eq!(
                compound_acceptance_value(class_miss_token, accepting_score, live_state),
                0
            );
        }
    }

    #[test]
    fn compound_cpu_oracle_checked_emits_one_word_per_item() {
        let tokens = vec![0, 1, 2, 3];
        let scores = vec![700, 700, 700, 700];
        let states = vec![1, 3, 5, 7];

        let output = compound_cpu_oracle_checked(&tokens, &scores, &states).unwrap();

        assert_eq!(output.len(), tokens.len());
    }

    #[test]
    fn compound_cpu_oracle_checked_rejects_silent_truncation() {
        let error = compound_cpu_oracle_checked(&[1, 2, 3], &[700, 701], &[1, 3, 5])
            .expect_err("Fix: mismatched compound inputs must never truncate");

        assert!(
            error
                .to_string()
                .contains("compound fused filter input length mismatch"),
            "{error}"
        );
    }
}

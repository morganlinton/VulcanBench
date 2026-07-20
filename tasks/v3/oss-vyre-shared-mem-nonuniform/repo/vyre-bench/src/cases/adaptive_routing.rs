//! `runtime.adaptive_routing.gpu_resident.1m` - GPU-side workload routing.
//!
//! This benchmark models a scheduler that classifies one million independent
//! work items into skip/fast/deep/escalate routes using only resident GPU
//! state. It is deliberately not an arithmetic microbenchmark: the useful work
//! is per-item decisioning that would usually be orchestrated on the CPU
//! between kernels.

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
const ROUTE_SALT: u32 = 0x9E37_79B9;
const RISK_MASK: u32 = 0x3ff;
const SUITES: &[SuiteKind] = &[
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

pub struct GpuResidentAdaptiveRouting;

struct AdaptiveRoutingPrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    escalated_count: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for GpuResidentAdaptiveRouting {
    fn id(&self) -> BenchId {
        BenchId("runtime.adaptive_routing.gpu_resident.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "GPU Resident Adaptive Routing 1M".to_string(),
            description:
                "Classify one million resident work items into skip/fast/deep/escalate routes without CPU orchestration"
                    .to_string(),
            tags: vec![
                "runtime".to_string(),
                "adaptive-routing".to_string(),
                "resident".to_string(),
                "scheduler".to_string(),
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
            feature_set: vec![
                "runtime.adaptive-routing".to_string(),
                "resident".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "GPU-resident adaptive workload routing",
            "rayon",
            "Rayon-parallel CPU scheduler over equivalent routing predicates",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = adaptive_routing_program();
        let (signals, histories, thresholds) = adaptive_routing_inputs();
        let inputs = vec![
            u32_bytes(&signals),
            u32_bytes(&histories),
            u32_bytes(&thresholds),
        ];
        let input_bytes_total = input_bytes_total(&inputs);
        let baseline_start = std::time::Instant::now();
        let baseline_words =
            adaptive_routing_cpu_oracle_checked(&signals, &histories, &thresholds)?;
        let baseline_wall_ns =
            u64::try_from(baseline_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let escalated_count = baseline_words
            .iter()
            .filter(|&&route| (route >> 24) == 3)
            .count() as u64;
        let baseline_output = u32_bytes(&baseline_words);
        let resident = ResidentInputSet::upload_with_zeroed_outputs_optional(
            ctx,
            &inputs,
            &[baseline_output.len()],
            "adaptive routing bench",
        )?;

        Ok(Box::new(AdaptiveRoutingPrepared {
            program,
            inputs,
            input_bytes_total,
            baseline_output,
            baseline_wall_ns,
            escalated_count,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<AdaptiveRoutingPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<AdaptiveRoutingPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "adaptive routing prepared payload type mismatch".to_string(),
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
                        name: "adaptive_routing_items".to_string(),
                        value: u64::from(ITEM_COUNT),
                    },
                    MetricPoint {
                        name: "adaptive_routing_predicates".to_string(),
                        value: 3,
                    },
                    MetricPoint {
                        name: "adaptive_routing_escalated".to_string(),
                        value: prepared.escalated_count,
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
            .downcast_ref::<AdaptiveRoutingPrepared>()
            .map(|prepared| (prepared.input_bytes_total, u64::from(ITEM_COUNT) * 4))
            .unwrap_or((u64::from(ITEM_COUNT) * 12, u64::from(ITEM_COUNT) * 4))
    }
}

fn adaptive_routing_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("signals", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::storage("histories", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::storage("thresholds", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ITEM_COUNT),
            BufferDecl::output("routes", 3, DataType::U32).with_count(ITEM_COUNT),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("tid", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("tid"), Expr::u32(ITEM_COUNT)),
                vec![
                    Node::let_bind("signal", Expr::load("signals", Expr::var("tid"))),
                    Node::let_bind("history", Expr::load("histories", Expr::var("tid"))),
                    Node::let_bind("threshold", Expr::load("thresholds", Expr::var("tid"))),
                    Node::let_bind(
                        "risk",
                        Expr::add(
                            Expr::bitand(
                                Expr::bitxor(
                                    Expr::var("signal"),
                                    Expr::mul(Expr::var("history"), Expr::u32(ROUTE_SALT)),
                                ),
                                Expr::u32(RISK_MASK),
                            ),
                            Expr::bitand(Expr::var("history"), Expr::u32(0xff)),
                        ),
                    ),
                    Node::let_bind("hot", Expr::ge(Expr::var("risk"), Expr::var("threshold"))),
                    Node::let_bind(
                        "unstable",
                        Expr::ge(
                            Expr::bitand(
                                Expr::shr(Expr::var("history"), Expr::u32(16)),
                                Expr::u32(7),
                            ),
                            Expr::u32(4),
                        ),
                    ),
                    Node::let_bind(
                        "escalate",
                        Expr::and(Expr::var("hot"), Expr::var("unstable")),
                    ),
                    Node::let_bind(
                        "route",
                        Expr::select(
                            Expr::var("escalate"),
                            Expr::u32(3),
                            Expr::select(
                                Expr::var("hot"),
                                Expr::u32(2),
                                Expr::select(Expr::var("unstable"), Expr::u32(1), Expr::u32(0)),
                            ),
                        ),
                    ),
                    Node::store(
                        "routes",
                        Expr::var("tid"),
                        Expr::bitor(
                            Expr::shl(Expr::var("route"), Expr::u32(24)),
                            Expr::bitand(Expr::var("risk"), Expr::u32(0x00ff_ffff)),
                        ),
                    ),
                ],
            ),
        ],
    )
}

fn adaptive_routing_inputs() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut signals = Vec::with_capacity(ITEM_COUNT as usize);
    let mut histories = Vec::with_capacity(ITEM_COUNT as usize);
    let mut thresholds = Vec::with_capacity(ITEM_COUNT as usize);
    for index in 0..ITEM_COUNT {
        let signal = mix32(index ^ 0x4D59_5DF4);
        let history = mix32(index.wrapping_mul(31).wrapping_add(0xA5A5_5A5A));
        let threshold = 320 + (mix32(index ^ 0x517C_C1B7) & 511);
        signals.push(signal);
        histories.push(history);
        thresholds.push(threshold);
    }
    (signals, histories, thresholds)
}

fn adaptive_routing_cpu_oracle(signals: &[u32], histories: &[u32], thresholds: &[u32]) -> Vec<u32> {
    signals
        .par_iter()
        .zip(histories.par_iter())
        .zip(thresholds.par_iter())
        .map(|((&signal, &history), &threshold)| adaptive_route_word(signal, history, threshold))
        .collect()
}

fn adaptive_routing_cpu_oracle_checked(
    signals: &[u32],
    histories: &[u32],
    thresholds: &[u32],
) -> Result<Vec<u32>, BenchError> {
    if signals.len() != histories.len() || signals.len() != thresholds.len() {
        return Err(BenchError::ExecutionFailed(format!(
            "adaptive routing input length mismatch: signals={}, histories={}, thresholds={}. Fix: generate equal-length streams before building the CPU oracle.",
            signals.len(),
            histories.len(),
            thresholds.len()
        )));
    }
    Ok(adaptive_routing_cpu_oracle(signals, histories, thresholds))
}

fn adaptive_route_word(signal: u32, history: u32, threshold: u32) -> u32 {
    let risk = ((signal ^ history.wrapping_mul(ROUTE_SALT)) & RISK_MASK) + (history & 0xff);
    let hot = risk >= threshold;
    let unstable = ((history >> 16) & 7) >= 4;
    let route = if hot && unstable {
        3
    } else if hot {
        2
    } else if unstable {
        1
    } else {
        0
    };
    (route << 24) | (risk & 0x00ff_ffff)
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

inventory::submit! {
    &GpuResidentAdaptiveRouting as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_route_vectors_cover_every_decision_class() {
        let mut seen = [false; 4];
        for index in 0..12_288_u32 {
            let signal = mix32(index);
            let history = mix32(index ^ 0xCAFE_BABE);
            let threshold = 128 + (index & 767);
            let word = adaptive_route_word(signal, history, threshold);
            let route = (word >> 24) as usize;

            assert!(route < seen.len());
            assert!((word & 0x00ff_ffff) <= RISK_MASK + 0xff);
            seen[route] = true;
        }

        assert_eq!(seen, [true, true, true, true]);
    }

    #[test]
    fn hand_built_routes_pin_priority_encoding() {
        let history_stable = 0_u32;
        let history_unstable = 4 << 16;
        let signal = 0;
        let cold_threshold = 2048;
        let hot_threshold = 0;

        assert_eq!(
            adaptive_route_word(signal, history_stable, cold_threshold) >> 24,
            0
        );
        assert_eq!(
            adaptive_route_word(signal, history_unstable, cold_threshold) >> 24,
            1
        );
        assert_eq!(
            adaptive_route_word(signal, history_stable, hot_threshold) >> 24,
            2
        );
        assert_eq!(
            adaptive_route_word(signal, history_unstable, hot_threshold) >> 24,
            3
        );
    }

    #[test]
    fn adaptive_routing_oracle_checked_rejects_silent_truncation() {
        let error = adaptive_routing_cpu_oracle_checked(&[1, 2, 3], &[4, 5, 6], &[7, 8])
            .expect_err("Fix: mismatched adaptive routing inputs must never truncate");

        assert!(
            error
                .to_string()
                .contains("adaptive routing input length mismatch"),
            "{error}"
        );
    }
}

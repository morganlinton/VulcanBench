use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{input_bytes_total, ResidentInputSet};
use crate::api::suite::SuiteKind;
use crate::cases::queue_closure_profile::validate_queue_closure_wave_profile;
use std::time::Instant;
use vyre_foundation::ir::Program;
use vyre_primitives::graph::csr_frontier_queue::frontier_queue_len_init;
use vyre_primitives::graph::csr_queue_delta::{
    csr_queue_delta_enqueue, csr_queue_delta_strided_dispatch_grid,
    csr_queue_delta_strided_enqueue, CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE,
};

use super::super::closure::CLOSURE_MAX_ITERS;
use super::super::fixture::{
    build_ifds_skewed_fixture, ifds_skewed_closure_oracle, IfdsSkewedStats, IFDS_REACH_MASK,
    NODE_COUNT,
};
use super::ifds_queue_should_use_row_strided;

mod metrics;
mod sequence;
mod support;

use metrics::{queue_closure_baseline_metric_points, queue_closure_metric_points};
use sequence::dispatch_resident_queue_closure_sequence;
#[cfg(not(test))]
use support::ifds_skewed_queue_closure_oracle;
#[cfg(test)]
pub(in crate::cases::dataflow_irregular) use support::ifds_skewed_queue_closure_oracle;
pub(in crate::cases::dataflow_irregular) use support::{
    ifds_queue_closure_inputs, ifds_queue_closure_reset_program,
};

const QUEUE_CLOSURE_SUITES: &[SuiteKind] = &[
    SuiteKind::Smoke,
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];
const QUEUE_CLOSURE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_SEED_FRONTIER_INDEX: usize = 0;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_SEED_QUEUE_INDEX: usize = 1;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_SEED_LEN_INDEX: usize = 2;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_QUEUE_A_INDEX: usize = 3;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_LEN_A_INDEX: usize = 4;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_QUEUE_B_INDEX: usize = 5;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_LEN_B_INDEX: usize = 6;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_EDGE_OFFSETS_INDEX: usize = 7;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_EDGE_TARGETS_INDEX: usize = 8;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_EDGE_KIND_INDEX: usize = 9;
pub(in crate::cases::dataflow_irregular) const QUEUE_CLOSURE_ACCUMULATOR_INDEX: usize = 10;

pub(in crate::cases::dataflow_irregular) struct DataflowIfdsSkewedQueueClosurePrepared {
    pub(in crate::cases::dataflow_irregular) reset_program: Program,
    pub(in crate::cases::dataflow_irregular) clear_len_program: Program,
    pub(in crate::cases::dataflow_irregular) delta_program: Program,
    pub(in crate::cases::dataflow_irregular) delta_grid: [u32; 3],
    pub(in crate::cases::dataflow_irregular) row_strided_delta: bool,
    pub(in crate::cases::dataflow_irregular) inputs: Vec<Vec<u8>>,
    pub(in crate::cases::dataflow_irregular) input_bytes_total: u64,
    pub(in crate::cases::dataflow_irregular) baseline_output: Vec<u8>,
    pub(in crate::cases::dataflow_irregular) baseline_wall_ns: u64,
    pub(in crate::cases::dataflow_irregular) stats: IfdsSkewedStats,
    pub(in crate::cases::dataflow_irregular) queue_capacity: u32,
    pub(in crate::cases::dataflow_irregular) seed_queue_len: u32,
    pub(in crate::cases::dataflow_irregular) closure_iterations: u32,
    pub(in crate::cases::dataflow_irregular) closure_changed: u32,
    pub(in crate::cases::dataflow_irregular) total_queue_pops: u64,
    pub(in crate::cases::dataflow_irregular) max_wave_queue_len: u32,
    pub(in crate::cases::dataflow_irregular) wave_queue_lengths: Vec<u32>,
    pub(in crate::cases::dataflow_irregular) resident: Option<ResidentInputSet>,
}

pub(in crate::cases::dataflow_irregular) fn ifds_queue_closure_delta_lanes_per_source(
    row_strided_delta: bool,
) -> u32 {
    if row_strided_delta {
        CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE
    } else {
        1
    }
}

/// Queue-driven IFDS closure seeded from a sparse queue.
struct DataflowIfdsSkewedQueueClosure;

impl BenchCase for DataflowIfdsSkewedQueueClosure {
    fn id(&self) -> BenchId {
        BenchId("dataflow.ifds.skewed.queue_closure.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Dataflow IFDS Skewed Queue Closure 1M".to_string(),
            description: "Sparse-delta IFDS closure over a million-node skewed exploded-supergraph using a pre-materialized seed queue and GPU-resident ping-pong active queues".to_string(),
            tags: vec![
                "dataflow".to_string(),
                "ifds".to_string(),
                "graph".to_string(),
                "csr".to_string(),
                "frontier-queue".to_string(),
                "delta-queue".to_string(),
                "seed-queue".to_string(),
                "closure".to_string(),
                "skewed-degree".to_string(),
                "irregular".to_string(),
                "resident".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-primitives".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        QUEUE_CLOSURE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(128 * 1024 * 1024),
            min_input_bytes: Some(u64::from(NODE_COUNT) * 20),
            feature_set: vec![
                "dataflow".to_string(),
                "ifds".to_string(),
                "skewed-csr".to_string(),
                "frontier-queue".to_string(),
                "delta-queue".to_string(),
                "seed-queue".to_string(),
                "resident-sequence".to_string(),
            ],
        }
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<DataflowIfdsSkewedQueueClosurePrepared>()
            .map(|prepared| {
                (
                    prepared.input_bytes_total,
                    prepared.baseline_output.len() as u64,
                )
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_ifds_skewed_queue_closure(Some(ctx))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<DataflowIfdsSkewedQueueClosurePrepared>()
            .map(|prepared| &prepared.delta_program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<DataflowIfdsSkewedQueueClosurePrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared IFDS queue-closure payload had the wrong type".to_string(),
                )
            })?;
        if ctx.dispatch_config.workgroup_override.is_some() {
            return Err(BenchError::ExecutionFailed(
                "IFDS queue closure uses mixed workgroups across reset, seed, clear, and delta kernels. Fix: run without a workgroup override."
                    .to_string(),
            ));
        }

        let resident = prepared.resident.as_ref().ok_or_else(|| {
            BenchError::EnvironmentInvalid(
                "IFDS queue closure requires resident GPU buffers. Fix: run on a backend with resident sequence support."
                    .to_string(),
            )
        })?;
        let sequence = dispatch_resident_queue_closure_sequence(ctx, prepared, resident)?;
        let output_bytes = sequence.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let custom = queue_closure_metric_points(prepared, sequence.wall_ns, true);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(sequence.wall_ns),
                dispatch_ns: None,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(0),
                bytes_written: Some(output_bytes),
                bytes_touched: Some(output_bytes),
                custom,
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                custom: queue_closure_baseline_metric_points(prepared),
                ..Default::default()
            }),
            outputs: sequence.outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

pub(in crate::cases::dataflow_irregular) fn prepare_ifds_skewed_queue_closure(
    ctx: Option<&BenchContext>,
) -> Result<DataflowIfdsSkewedQueueClosurePrepared, BenchError> {
    let fixture = build_ifds_skewed_fixture(NODE_COUNT)?;
    let seed_queue_len = u32::try_from(fixture.stats.active_sources).map_err(|_| {
        BenchError::EnvironmentInvalid(format!(
            "IFDS queue closure active source count {} exceeds u32 indexing. Fix: split the seed queue.",
            fixture.stats.active_sources
        ))
    })?;
    let baseline_start = Instant::now();
    let oracle =
        ifds_skewed_queue_closure_oracle(&fixture, CLOSURE_MAX_ITERS, fixture.stats.nodes)?;
    let baseline_wall_ns = baseline_start
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    let queue_capacity = oracle.max_wave_queue_len.max(seed_queue_len).max(1);
    validate_queue_closure_wave_profile(
        "IFDS",
        &oracle.wave_queue_lengths,
        oracle.iterations,
        oracle.total_queue_pops,
        oracle.max_wave_queue_len,
        queue_capacity,
    )?;
    let reset_program = ifds_queue_closure_reset_program(
        fixture.stats.frontier_words,
        seed_queue_len,
        queue_capacity,
    );
    let clear_len_program = frontier_queue_len_init("queue_len");
    let row_strided_delta = ifds_queue_should_use_row_strided(fixture.stats.max_degree);
    let (delta_program, delta_grid) = if row_strided_delta {
        (
            csr_queue_delta_strided_enqueue(
                "active_queue",
                "active_len",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "accumulator",
                "next_queue",
                "next_len",
                fixture.stats.nodes,
                fixture.stats.edges,
                queue_capacity,
                queue_capacity,
                IFDS_REACH_MASK,
            ),
            csr_queue_delta_strided_dispatch_grid(queue_capacity),
        )
    } else {
        (
            csr_queue_delta_enqueue(
                "active_queue",
                "active_len",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "accumulator",
                "next_queue",
                "next_len",
                fixture.stats.nodes,
                fixture.stats.edges,
                queue_capacity,
                queue_capacity,
                IFDS_REACH_MASK,
            ),
            [
                queue_capacity
                    .div_ceil(QUEUE_CLOSURE_WORKGROUP_SIZE[0])
                    .max(1),
                1,
                1,
            ],
        )
    };

    let full_oracle = ifds_skewed_closure_oracle(&fixture, CLOSURE_MAX_ITERS);
    if oracle.output != full_oracle.output {
        return Err(BenchError::CorrectnessViolation(
            "IFDS queue-closure oracle disagreed with full bitset closure oracle".to_string(),
        ));
    }

    let mut stats = fixture.stats;
    stats.output_words_set = oracle.output.iter().filter(|word| **word != 0).count() as u64;
    let inputs = ifds_queue_closure_inputs(&fixture, queue_capacity)?;
    let input_bytes_total = input_bytes_total(&inputs);
    let resident = ctx
        .map(|ctx| ResidentInputSet::upload_optional(ctx, &inputs, "dataflow IFDS queue closure"))
        .transpose()?
        .flatten();

    Ok(DataflowIfdsSkewedQueueClosurePrepared {
        reset_program,
        clear_len_program,
        delta_program,
        delta_grid,
        row_strided_delta,
        inputs,
        input_bytes_total,
        baseline_output: vyre_primitives::wire::pack_u32_slice(&oracle.output),
        baseline_wall_ns,
        stats,
        queue_capacity,
        seed_queue_len,
        closure_iterations: oracle.iterations,
        closure_changed: oracle.changed,
        total_queue_pops: oracle.total_queue_pops,
        max_wave_queue_len: oracle.max_wave_queue_len,
        wave_queue_lengths: oracle.wave_queue_lengths,
        resident,
    })
}

inventory::submit! {
    &DataflowIfdsSkewedQueueClosure as &'static dyn BenchCase
}

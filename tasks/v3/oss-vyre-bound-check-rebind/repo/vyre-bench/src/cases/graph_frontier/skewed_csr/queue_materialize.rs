use std::time::Instant;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{input_bytes_total, ResidentInputSet};
use vyre_foundation::ir::Program;
use vyre_primitives::graph::csr_frontier_queue::{
    csr_queue_forward_traverse, frontier_queue_len_init, frontier_words_to_queue_clear_out_parallel,
};
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_low_dispatch_grid, csr_queue_split_low_forward_traverse,
    csr_queue_split_mixed_logical_lanes,
};
use vyre_primitives::graph::csr_queue_strided::{
    csr_queue_strided_forward_dispatch_grid, csr_queue_strided_forward_traverse,
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE,
};

use super::metrics::{skewed_csr_baseline_metric_points, skewed_csr_queue_metric_points};
use super::support::{
    build_skewed_csr_fixture, skewed_csr_active_high_degree_sources, skewed_csr_cpu_oracle,
    skewed_csr_queue_capacity, skewed_csr_queue_inputs, SkewedCsrStats, CSR_ALLOW_MASK,
    CSR_NODE_COUNT, SUITES,
};

#[path = "queue_sequence.rs"]
mod queue_sequence;

use queue_sequence::{dispatch_host_queue_sequence, dispatch_resident_queue_sequence};

pub(super) const QUEUE_FRONTIER_IN_INDEX: usize = 0;
pub(super) const QUEUE_ACTIVE_QUEUE_INDEX: usize = 1;
pub(super) const QUEUE_LEN_INDEX: usize = 2;
pub(super) const QUEUE_EDGE_OFFSETS_INDEX: usize = 3;
pub(super) const QUEUE_EDGE_TARGETS_INDEX: usize = 4;
pub(super) const QUEUE_EDGE_KIND_INDEX: usize = 5;
pub(super) const QUEUE_FRONTIER_OUT_INDEX: usize = 6;
pub(super) const QUEUE_HIGH_QUEUE_INDEX: usize = 7;
pub(super) const QUEUE_HIGH_LEN_INDEX: usize = 8;
pub(super) const QUEUE_RESET_GRID: [u32; 3] = [1, 1, 1];
pub(super) const GRAPH_QUEUE_ROW_STRIDED_MIN_DEGREE: u32 =
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE
        .saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);
pub(super) const GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD: u32 =
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE * 2;

pub(super) struct GraphCsrSkewedQueuePrepared {
    pub(super) reset_program: Program,
    pub(super) queue_program: Program,
    pub(super) traverse_program: Program,
    pub(super) traverse_grid: [u32; 3],
    pub(super) row_strided_traverse: bool,
    pub(super) split_high_degree_traverse: bool,
    pub(super) high_traverse_program: Option<Program>,
    pub(super) high_traverse_grid: [u32; 3],
    pub(super) high_degree_queue_capacity: u32,
    pub(super) traverse_logical_lanes: u64,
    pub(super) inputs: Vec<Vec<u8>>,
    pub(super) input_bytes_total: u64,
    pub(super) baseline_output: Vec<u8>,
    pub(super) baseline_wall_ns: u64,
    pub(super) stats: SkewedCsrStats,
    pub(super) queue_capacity: u32,
    pub(super) resident: Option<ResidentInputSet>,
}

struct GraphCsrSkewedQueueMaterializeStep;

impl BenchCase for GraphCsrSkewedQueueMaterializeStep {
    fn id(&self) -> BenchId {
        BenchId("primitives.graph.csr_skewed_queue_materialize.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Skewed CSR Queue Materialize 1M".to_string(),
            description: "GPU-resident packed-frontier queue materialization plus queue-driven CSR expansion over a million-node skewed graph".to_string(),
            tags: vec![
                "graph".to_string(),
                "frontier".to_string(),
                "csr".to_string(),
                "frontier-queue".to_string(),
                "skewed-degree".to_string(),
                "irregular".to_string(),
                "resident".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Macro,
            determinism: crate::api::case::DeterminismClass::Deterministic,
            owner_crate: "vyre-primitives".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(96 * 1024 * 1024),
            min_input_bytes: Some(u64::from(CSR_NODE_COUNT) * 12),
            feature_set: vec![
                "graph.csr".to_string(),
                "graph.frontier.bitset".to_string(),
                "graph.frontier.queue".to_string(),
                "graph.skewed-degree".to_string(),
                "resident-sequence".to_string(),
            ],
        }
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<GraphCsrSkewedQueuePrepared>()
            .map(|prepared| {
                (
                    prepared.input_bytes_total,
                    prepared.baseline_output.len() as u64,
                )
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_skewed_csr_queue_materialize_step(Some(
            ctx,
        ))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<GraphCsrSkewedQueuePrepared>()
            .map(|prepared| &prepared.traverse_program)
    }

    fn workload_fingerprint_bytes(&self, prepared: &PreparedCase) -> Option<[u8; 32]> {
        prepared
            .downcast_ref::<GraphCsrSkewedQueuePrepared>()
            .map(graph_queue_materialize_sequence_fingerprint)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<GraphCsrSkewedQueuePrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared skewed CSR queue payload had the wrong type".to_string(),
                )
            })?;
        let workgroup = prepared.queue_program.workgroup_size();
        if workgroup.contains(&0) {
            return Err(BenchError::ExecutionFailed(format!(
                "skewed CSR queue benchmark received invalid workgroup {:?}. Fix: use positive dispatch dimensions.",
                workgroup
            )));
        }
        if let Some(override_workgroup) = ctx.dispatch_config.workgroup_override {
            if override_workgroup != workgroup {
                return Err(BenchError::ExecutionFailed(format!(
                    "skewed CSR queue resident sequence uses program workgroup {:?}, but received override {:?}. Fix: run the queue sequence without a workgroup override or rebuild all sequence programs.",
                    workgroup, override_workgroup
                )));
            }
        }

        let sequence = if let Some(resident) = prepared.resident.as_ref() {
            dispatch_resident_queue_sequence(ctx, prepared, resident, workgroup)?
        } else {
            dispatch_host_queue_sequence(ctx, prepared, workgroup)?
        };
        let output_bytes = sequence.outputs.iter().map(Vec::len).sum::<usize>() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(sequence.wall_ns),
                dispatch_ns: sequence.dispatch_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(sequence.bytes_read),
                bytes_written: Some(sequence.bytes_written),
                bytes_touched: Some(sequence.bytes_read.saturating_add(sequence.bytes_written)),
                custom: skewed_csr_queue_metric_points(
                    prepared.stats,
                    prepared.queue_capacity,
                    prepared.high_degree_queue_capacity,
                    prepared.traverse_logical_lanes,
                    prepared.baseline_wall_ns,
                    sequence.wall_ns,
                    sequence.resident_used,
                    workgroup[0],
                    prepared.row_strided_traverse,
                    prepared.split_high_degree_traverse,
                    GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
                    true,
                    QUEUE_RESET_GRID.into_iter().product(),
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                custom: skewed_csr_baseline_metric_points(prepared.stats),
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

pub(super) fn prepare_skewed_csr_queue_materialize_step(
    ctx: Option<&BenchContext>,
) -> Result<GraphCsrSkewedQueuePrepared, BenchError> {
    let fixture = build_skewed_csr_fixture(CSR_NODE_COUNT)?;
    let queue_capacity = skewed_csr_queue_capacity(fixture.stats.active_sources)?;
    let high_degree_queue_capacity =
        skewed_csr_active_high_degree_sources(&fixture, GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD)?;
    let reset_program = frontier_queue_len_init("queue_len");
    let queue_program = frontier_words_to_queue_clear_out_parallel(
        "frontier_in",
        "active_queue",
        "queue_len",
        "frontier_out",
        fixture.stats.node_count,
        queue_capacity,
    );
    let traverse_plan = graph_queue_traverse_plan(
        fixture.stats.max_degree,
        fixture.stats.node_count,
        fixture.stats.edge_count,
        queue_capacity,
        high_degree_queue_capacity,
    );

    let baseline_start = Instant::now();
    let oracle = skewed_csr_cpu_oracle(&fixture);
    let baseline_wall_ns = baseline_start
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    let mut stats = fixture.stats;
    stats.allowed_edges_from_active = oracle.allowed_edges_from_active;
    stats.output_words_set = oracle.output_words_set;

    let inputs = skewed_csr_queue_inputs(&fixture, queue_capacity, high_degree_queue_capacity)?;
    let input_bytes_total = input_bytes_total(&inputs);
    let resident = ctx
        .map(|ctx| ResidentInputSet::upload_optional(ctx, &inputs, "skewed CSR graph queue"))
        .transpose()?
        .flatten();

    Ok(GraphCsrSkewedQueuePrepared {
        reset_program,
        queue_program,
        traverse_program: traverse_plan.program,
        traverse_grid: traverse_plan.grid,
        row_strided_traverse: traverse_plan.row_strided,
        split_high_degree_traverse: traverse_plan.split_high_degree,
        high_traverse_program: traverse_plan.high_program,
        high_traverse_grid: traverse_plan.high_grid,
        high_degree_queue_capacity,
        traverse_logical_lanes: traverse_plan.logical_lanes,
        inputs,
        input_bytes_total,
        baseline_output: vyre_primitives::wire::pack_u32_slice(&oracle.output),
        baseline_wall_ns,
        stats,
        queue_capacity,
        resident,
    })
}

pub(super) fn graph_queue_materialize_sequence_fingerprint(
    prepared: &GraphCsrSkewedQueuePrepared,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-bench:primitives.graph.csr_skewed_queue_materialize.sequence:v2");
    for fingerprint in [
        prepared.reset_program.fingerprint(),
        prepared.queue_program.fingerprint(),
        prepared.traverse_program.fingerprint(),
    ] {
        hasher.update(&fingerprint);
    }
    if let Some(program) = prepared.high_traverse_program.as_ref() {
        hasher.update(&program.fingerprint());
    }
    for value in QUEUE_RESET_GRID
        .into_iter()
        .chain(prepared.queue_program.workgroup_size())
        .chain(prepared.traverse_grid)
        .chain(prepared.high_traverse_grid)
        .chain([
            prepared.high_degree_queue_capacity,
            u32::from(prepared.split_high_degree_traverse),
        ])
    {
        hasher.update(&value.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

struct GraphQueueTraversePlan {
    program: Program,
    grid: [u32; 3],
    row_strided: bool,
    split_high_degree: bool,
    high_program: Option<Program>,
    high_grid: [u32; 3],
    logical_lanes: u64,
}

fn graph_queue_traverse_plan(
    max_degree: u32,
    node_count: u32,
    edge_count: u32,
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
) -> GraphQueueTraversePlan {
    if graph_queue_should_use_split_high_degree(queue_capacity, high_degree_queue_capacity) {
        let program = csr_queue_split_low_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            "high_queue",
            "high_len",
            node_count,
            edge_count,
            queue_capacity,
            high_degree_queue_capacity,
            GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
            CSR_ALLOW_MASK,
        );
        let high_program = csr_queue_strided_forward_traverse(
            "high_queue",
            "high_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            node_count,
            edge_count,
            high_degree_queue_capacity,
            CSR_ALLOW_MASK,
        );
        return GraphQueueTraversePlan {
            program,
            grid: csr_queue_split_low_dispatch_grid(queue_capacity),
            row_strided: true,
            split_high_degree: true,
            high_program: Some(high_program),
            high_grid: csr_queue_strided_forward_dispatch_grid(high_degree_queue_capacity),
            logical_lanes: csr_queue_split_mixed_logical_lanes(
                queue_capacity,
                high_degree_queue_capacity,
            ),
        };
    }

    let row_strided = graph_queue_should_use_row_strided(max_degree);
    let program = if row_strided {
        csr_queue_strided_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            node_count,
            edge_count,
            queue_capacity,
            CSR_ALLOW_MASK,
        )
    } else {
        csr_queue_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            node_count,
            edge_count,
            queue_capacity,
            CSR_ALLOW_MASK,
        )
    };
    let grid = if row_strided {
        csr_queue_strided_forward_dispatch_grid(queue_capacity)
    } else {
        [queue_capacity.div_ceil(256).max(1), 1, 1]
    };

    GraphQueueTraversePlan {
        program,
        grid,
        row_strided,
        split_high_degree: false,
        high_program: None,
        high_grid: [1, 1, 1],
        logical_lanes: graph_queue_traverse_logical_lanes(queue_capacity, row_strided),
    }
}

pub(super) const fn graph_queue_should_use_split_high_degree(
    queue_capacity: u32,
    high_degree_queue_capacity: u32,
) -> bool {
    high_degree_queue_capacity > 0 && high_degree_queue_capacity < queue_capacity
}

pub(super) const fn graph_queue_should_use_row_strided(max_degree: u32) -> bool {
    max_degree >= GRAPH_QUEUE_ROW_STRIDED_MIN_DEGREE
}

pub(super) const fn graph_queue_traverse_logical_lanes(
    queue_capacity: u32,
    row_strided_traverse: bool,
) -> u64 {
    if row_strided_traverse {
        (queue_capacity as u64).saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE as u64)
    } else {
        queue_capacity as u64
    }
}

inventory::submit! {
    &GraphCsrSkewedQueueMaterializeStep as &'static dyn BenchCase
}

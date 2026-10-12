use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use vyre_foundation::ir::Program;
use vyre_primitives::graph::program_graph::ProgramGraphShape;

mod metrics;
mod queue_closure;
mod queue_materialize;
mod support;
#[cfg(test)]
mod tests;

use metrics::{skewed_csr_baseline_metric_points, skewed_csr_metric_points};
use support::{
    build_skewed_csr_fixture, skewed_csr_cpu_oracle, skewed_csr_inputs, SkewedCsrStats,
    CSR_ALLOW_MASK, CSR_NODE_COUNT, SUITES,
};

struct GraphCsrSkewedPrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    stats: SkewedCsrStats,
    resident: Option<ResidentInputSet>,
}

/// Million-node packed-bitset CSR expansion with skewed row degrees.
struct GraphCsrSkewedFrontierStep;

impl BenchCase for GraphCsrSkewedFrontierStep {
    fn id(&self) -> BenchId {
        BenchId("primitives.graph.csr_skewed_frontier.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Skewed CSR Bitset Frontier 1M".to_string(),
            description: "Packed-bitset CSR frontier expansion over a million-node skewed-degree graph with edge-kind filtering and atomic output bits".to_string(),
            tags: vec![
                "graph".to_string(),
                "frontier".to_string(),
                "csr".to_string(),
                "bitset".to_string(),
                "skewed-degree".to_string(),
                "atomic".to_string(),
                "irregular".to_string(),
            ],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
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
            min_input_bytes: Some(u64::from(CSR_NODE_COUNT) * 20),
            feature_set: vec![
                "graph.csr".to_string(),
                "graph.frontier.bitset".to_string(),
                "graph.skewed-degree".to_string(),
            ],
        }
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<GraphCsrSkewedPrepared>()
            .map(|prepared| {
                (
                    prepared.input_bytes_total,
                    prepared.baseline_output.len() as u64,
                )
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_skewed_csr_case(Some(ctx))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<GraphCsrSkewedPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<GraphCsrSkewedPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared skewed CSR graph payload had the wrong type".to_string(),
                )
            })?;

        let mut dispatch_config = ctx.dispatch_config.clone();
        let workgroup = dispatch_config
            .workgroup_override
            .unwrap_or_else(|| prepared.program.workgroup_size());
        if workgroup.contains(&0) {
            return Err(BenchError::ExecutionFailed(format!(
                "skewed CSR graph benchmark received invalid workgroup {:?}. Fix: use positive dispatch dimensions.",
                workgroup
            )));
        }
        dispatch_config.grid_override.get_or_insert([
            prepared.stats.node_count.div_ceil(workgroup[0]),
            1,
            1,
        ]);

        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;
        let output_bytes_total = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(
            prepared.input_bytes_total,
            output_bytes_total,
            resident_used,
        );

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes_total),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: skewed_csr_metric_points(
                    prepared.stats,
                    prepared.baseline_wall_ns,
                    timed.wall_ns,
                    resident_used,
                    workgroup[0],
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
            outputs: timed.outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn prepare_skewed_csr_case(
    ctx: Option<&BenchContext>,
) -> Result<GraphCsrSkewedPrepared, BenchError> {
    let fixture = build_skewed_csr_fixture(CSR_NODE_COUNT)?;
    let shape = ProgramGraphShape::new(fixture.stats.node_count, fixture.stats.edge_count);
    let program = vyre_primitives::graph::csr_forward_traverse::csr_forward_traverse(
        shape,
        "frontier_in",
        "frontier_out",
        CSR_ALLOW_MASK,
    );

    let start_ref = std::time::Instant::now();
    let oracle = skewed_csr_cpu_oracle(&fixture);
    let baseline_wall_ns = start_ref.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    let mut stats = fixture.stats;
    stats.allowed_edges_from_active = oracle.allowed_edges_from_active;
    stats.output_words_set = oracle.output_words_set;

    let inputs = skewed_csr_inputs(&fixture);
    let input_bytes_total = input_bytes_total(&inputs);
    let resident = ctx
        .map(|ctx| ResidentInputSet::upload_optional(ctx, &inputs, "skewed CSR graph frontier"))
        .transpose()?
        .flatten();

    Ok(GraphCsrSkewedPrepared {
        program,
        inputs,
        input_bytes_total,
        baseline_output: vyre_primitives::wire::pack_u32_slice(&oracle.output),
        baseline_wall_ns,
        stats,
        resident,
    })
}

inventory::submit! {
    &GraphCsrSkewedFrontierStep as &'static dyn BenchCase
}

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use external_dataflow_engine::{
    ifds as dataflow_ifds, points_to as dataflow_points_to, reaching_def as dataflow_reaching_def,
};
use vyre_primitives::bitset::and::cpu_ref as bitset_and_cpu_ref;
use vyre_primitives::graph::csr_forward_traverse::{bitset_words, cpu_ref as csr_step_cpu_ref};
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

pub struct DataflowReachingDefBitset;
pub struct DataflowIfdsStep;
pub struct DataflowPointsToAliasStep;

const NODE_COUNT: u32 = 1_048_576;
const WORD_COUNT: usize = (NODE_COUNT as usize).div_ceil(32);
const GRAPH_NODE_COUNT: u32 = 262_144;
const GRAPH_EDGE_COUNT: u32 = GRAPH_NODE_COUNT - 1;
const GRAPH_WORD_COUNT: usize = bitset_words(GRAPH_NODE_COUNT) as usize;
const DATAFLOW_IFDS_ALLOW_MASK: u32 = edge_kind::ASSIGNMENT
    | edge_kind::CALL_ARG
    | edge_kind::RETURN
    | edge_kind::PHI
    | edge_kind::ALIAS
    | edge_kind::MEM_STORE
    | edge_kind::MEM_LOAD
    | edge_kind::MUT_REF;

struct DataflowBitsetPrepared {
    program: vyre_foundation::ir::Program,
    encoded_inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    resident: Option<ResidentInputSet>,
}

struct DataflowGraphPrepared {
    program: vyre_foundation::ir::Program,
    encoded_inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    resident: Option<ResidentInputSet>,
    workload_name: &'static str,
    active_sources: u64,
    max_out_degree: u32,
    allowed_edges: u64,
}

struct DataflowGraphFixture {
    nodes: Vec<u32>,
    edge_offsets: Vec<u32>,
    edge_targets: Vec<u32>,
    edge_kind_mask: Vec<u32>,
    node_tags: Vec<u32>,
    frontier_in: Vec<u32>,
    frontier_out_seed: Vec<u32>,
}

impl BenchCase for DataflowReachingDefBitset {
    fn id(&self) -> BenchId {
        BenchId("dataflow.reaching_def.bitset.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Dataflow Reaching-Def Bitset 1M".to_string(),
            description: "Reaching-definition query over a 1M-node packed bitset dataflow workload"
                .to_string(),
            tags: vec![
                "dataflow".to_string(),
                "reaching".to_string(),
                "reaching_def".to_string(),
                "bitset".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-primitives".to_string(),
        }
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some((WORD_COUNT * 12) as u64),
            feature_set: vec!["dataflow".to_string(), "bitset".to_string()],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_100x(
            "dataflow reaching-def packed bitset",
            "dataflow",
            "single-threaded packed u32 bitset intersection",
        ))
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        &[
            crate::api::suite::SuiteKind::Release,
            crate::api::suite::SuiteKind::Gpu,
            crate::api::suite::SuiteKind::Deep,
            crate::api::suite::SuiteKind::Honest,
        ]
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_reaching_def_bitset(Some(ctx))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        prepared
            .downcast_ref::<DataflowBitsetPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<DataflowBitsetPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "dataflow reaching-def prepared payload type mismatch".to_string(),
                )
            })?;
        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.encoded_inputs,
            &ctx.dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;

        let input_bytes = prepared.input_bytes_total;
        let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(input_bytes, output_bytes, resident_used);
        let wall_ns = timed.wall_ns;
        let device_ns = timed.device_ns.unwrap_or(wall_ns);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_touched: Some(accounting.bytes_touched),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                wall_throughput_gb_s: Some(gb_per_second(accounting.bytes_touched, wall_ns)),
                device_throughput_gb_s: Some(gb_per_second(accounting.bytes_touched, device_ns)),
                custom: vec![
                    MetricPoint {
                        name: "nodes".to_string(),
                        value: u64::from(NODE_COUNT),
                    },
                    MetricPoint {
                        name: "bitset_words".to_string(),
                        value: WORD_COUNT as u64,
                    },
                    MetricPoint {
                        name: "resident_buffers".to_string(),
                        value: u64::from(prepared.resident.is_some()),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some((WORD_COUNT * 8) as u64),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                bytes_touched: Some((WORD_COUNT * 12) as u64),
                bytes_read: Some((WORD_COUNT * 8) as u64),
                bytes_written: Some((WORD_COUNT * 4) as u64),
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        ((WORD_COUNT * 8) as u64, (WORD_COUNT * 4) as u64)
    }
}

fn prepare_reaching_def_bitset(
    ctx: Option<&BenchContext>,
) -> Result<DataflowBitsetPrepared, BenchError> {
    let (gen_kill_in, use_set, out_seed) = reaching_def_fixture_words();
    let encoded_inputs = vec![
        encode_u32_words(&gen_kill_in),
        encode_u32_words(&use_set),
        encode_u32_words(&out_seed),
    ];
    let input_bytes_total = input_bytes_total(&encoded_inputs);
    let program = dataflow_reaching_def::reaching_def(NODE_COUNT, "gen_kill_in", "use_set", "out");
    let baseline_start = std::time::Instant::now();
    let baseline_words = bitset_and_cpu_ref(&gen_kill_in, &use_set);
    let baseline_wall_ns = baseline_start
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    let baseline_output = encode_u32_words(&baseline_words);
    let resident = ctx
        .map(|ctx| ResidentInputSet::upload_optional(ctx, &encoded_inputs, "dataflow bitset"))
        .transpose()?
        .flatten();

    Ok(DataflowBitsetPrepared {
        program,
        encoded_inputs,
        input_bytes_total,
        baseline_output,
        baseline_wall_ns,
        resident,
    })
}

fn reaching_def_fixture_words() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut gen_kill_in = Vec::with_capacity(WORD_COUNT);
    let mut use_set = Vec::with_capacity(WORD_COUNT);
    for index in 0..WORD_COUNT {
        let x = index as u32;
        gen_kill_in.push(x.rotate_left(5) ^ 0xA5A5_5A5A);
        use_set.push(x.wrapping_mul(0x9E37_79B9).rotate_right(7) ^ 0x3C3C_C3C3);
    }
    let out_seed = vec![0; WORD_COUNT];
    (gen_kill_in, use_set, out_seed)
}

fn encode_u32_words(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn gb_per_second(bytes: u64, ns: u64) -> f64 {
    if ns == 0 {
        return 0.0;
    }
    bytes as f64 / ns as f64
}

inventory::submit! {
    &DataflowReachingDefBitset as &'static dyn BenchCase
}

impl BenchCase for DataflowIfdsStep {
    fn id(&self) -> BenchId {
        BenchId("dataflow.ifds.step.262k".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Dataflow IFDS Step 262K".to_string(),
            description:
                "One IFDS reachability step over a 262K-node exploded-supergraph-shaped CSR"
                    .to_string(),
            tags: vec![
                "dataflow".to_string(),
                "ifds".to_string(),
                "graph".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-primitives".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        DATAFLOW_RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        graph_requirements()
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "dataflow IFDS reachability step",
            "dataflow",
            "single-threaded CSR frontier propagation",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let shape = ProgramGraphShape::new(GRAPH_NODE_COUNT, GRAPH_EDGE_COUNT);
        Ok(Box::new(irregular_graph_prepared(
            dataflow_ifds::ifds_reach_step(shape, "frontier_in", "frontier_out"),
            "ifds_step",
            DATAFLOW_IFDS_ALLOW_MASK,
            Some(ctx),
        )?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        prepared
            .downcast_ref::<DataflowGraphPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        run_graph_step(ctx, prepared)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        graph_bytes_touched()
    }
}

impl BenchCase for DataflowPointsToAliasStep {
    fn id(&self) -> BenchId {
        BenchId("dataflow.points_to.alias.step.262k".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Dataflow Points-To Alias Step 262K".to_string(),
            description:
                "One Andersen points-to alias propagation step over a 262K-node constraint CSR"
                    .to_string(),
            tags: vec![
                "dataflow".to_string(),
                "points_to".to_string(),
                "points-to".to_string(),
                "alias".to_string(),
                "may_alias".to_string(),
                "graph".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-primitives".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        DATAFLOW_RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        graph_requirements()
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_10x(
            "dataflow points-to alias propagation step",
            "dataflow",
            "single-threaded Andersen subset frontier propagation",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let shape = ProgramGraphShape::new(GRAPH_NODE_COUNT, GRAPH_EDGE_COUNT);
        Ok(Box::new(irregular_graph_prepared(
            dataflow_points_to::andersen_points_to(shape, "frontier_in", "frontier_out"),
            "points_to_alias_step",
            u32::MAX,
            Some(ctx),
        )?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        prepared
            .downcast_ref::<DataflowGraphPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        run_graph_step(ctx, prepared)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        graph_bytes_touched()
    }
}

const DATAFLOW_RELEASE_SUITES: &[crate::api::suite::SuiteKind] = &[
    crate::api::suite::SuiteKind::Release,
    crate::api::suite::SuiteKind::Gpu,
    crate::api::suite::SuiteKind::Deep,
    crate::api::suite::SuiteKind::Honest,
];

fn graph_requirements() -> BenchRequirements {
    let (input_bytes, output_bytes) = graph_bytes_touched();
    BenchRequirements {
        needs_gpu: true,
        needs_network: false,
        min_vram_bytes: None,
        min_input_bytes: Some(input_bytes.saturating_add(output_bytes)),
        feature_set: vec![
            "dataflow".to_string(),
            "graph".to_string(),
            "dataflow".to_string(),
        ],
    }
}

fn irregular_graph_prepared(
    program: vyre_foundation::ir::Program,
    workload_name: &'static str,
    allow_mask: u32,
    ctx: Option<&BenchContext>,
) -> Result<DataflowGraphPrepared, BenchError> {
    let fixture = irregular_graph_fixture(GRAPH_NODE_COUNT, GRAPH_EDGE_COUNT);
    let encoded_inputs = vec![
        encode_u32_words(&fixture.nodes),
        encode_u32_words(&fixture.edge_offsets),
        encode_u32_words(&fixture.edge_targets),
        encode_u32_words(&fixture.edge_kind_mask),
        encode_u32_words(&fixture.node_tags),
        encode_u32_words(&fixture.frontier_in),
        encode_u32_words(&fixture.frontier_out_seed),
    ];
    let input_bytes_total = input_bytes_total(&encoded_inputs);
    let baseline_start = std::time::Instant::now();
    let mut baseline_words = graph_step_baseline_words(&fixture, GRAPH_NODE_COUNT, allow_mask);
    for (out, seed) in baseline_words
        .iter_mut()
        .zip(fixture.frontier_out_seed.iter())
    {
        *out |= *seed;
    }
    let baseline_wall_ns = baseline_start
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    let baseline_output = encode_u32_words(&baseline_words);
    let active_sources = bitset_popcount(&fixture.frontier_in);
    let max_out_degree = max_out_degree(&fixture.edge_offsets);
    let allowed_edges = fixture
        .edge_kind_mask
        .iter()
        .filter(|&&kind| (kind & allow_mask) != 0)
        .count() as u64;
    let resident = ctx
        .map(|ctx| ResidentInputSet::upload_optional(ctx, &encoded_inputs, "dataflow graph"))
        .transpose()?
        .flatten();

    Ok(DataflowGraphPrepared {
        program,
        encoded_inputs,
        input_bytes_total,
        baseline_output,
        baseline_wall_ns,
        resident,
        workload_name,
        active_sources,
        max_out_degree,
        allowed_edges,
    })
}

fn run_graph_step(
    ctx: &mut BenchContext,
    prepared: &mut PreparedCase,
) -> Result<BenchRun, BenchError> {
    let prepared = prepared
        .downcast_ref::<DataflowGraphPrepared>()
        .ok_or_else(|| {
            BenchError::ExecutionFailed("graph prepared payload type mismatch".to_string())
        })?;
    let dispatch = dispatch_program_timed(
        ctx,
        &prepared.program,
        prepared.resident.as_ref(),
        &prepared.encoded_inputs,
        &ctx.dispatch_config,
    )?;
    let resident_used = dispatch.resident_used;
    let timed = dispatch.timed;

    let input_bytes = prepared.input_bytes_total;
    let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
    let accounting = transfer_accounting(input_bytes, output_bytes, resident_used);
    let wall_ns = timed.wall_ns;
    let device_ns = timed.device_ns.unwrap_or(wall_ns);
    let baseline_output_bytes = prepared.baseline_output.len() as u64;

    Ok(BenchRun {
        metrics: BenchMetrics {
            wall_ns: Some(wall_ns),
            dispatch_ns: timed.device_ns,
            input_bytes: Some(input_bytes),
            output_bytes: Some(output_bytes),
            bytes_touched: Some(accounting.bytes_touched),
            bytes_read: Some(accounting.bytes_read),
            bytes_written: Some(accounting.bytes_written),
            wall_throughput_gb_s: Some(gb_per_second(accounting.bytes_touched, wall_ns)),
            device_throughput_gb_s: Some(gb_per_second(accounting.bytes_touched, device_ns)),
            custom: vec![
                MetricPoint {
                    name: "graph_nodes".to_string(),
                    value: u64::from(GRAPH_NODE_COUNT),
                },
                MetricPoint {
                    name: "graph_edges".to_string(),
                    value: u64::from(GRAPH_EDGE_COUNT),
                },
                MetricPoint {
                    name: "graph_active_sources".to_string(),
                    value: prepared.active_sources,
                },
                MetricPoint {
                    name: "graph_max_out_degree".to_string(),
                    value: u64::from(prepared.max_out_degree),
                },
                MetricPoint {
                    name: "graph_allowed_edges".to_string(),
                    value: prepared.allowed_edges,
                },
                MetricPoint {
                    name: prepared.workload_name.to_string(),
                    value: 1,
                },
                MetricPoint {
                    name: "resident_buffers".to_string(),
                    value: u64::from(prepared.resident.is_some()),
                },
            ],
            ..Default::default()
        },
        baseline_metrics: Some(BenchMetrics {
            wall_ns: Some(prepared.baseline_wall_ns),
            input_bytes: Some(input_bytes),
            output_bytes: Some(baseline_output_bytes),
            bytes_touched: Some(input_bytes.saturating_add(baseline_output_bytes)),
            bytes_read: Some(input_bytes),
            bytes_written: Some(baseline_output_bytes),
            ..Default::default()
        }),
        outputs: timed.outputs,
        baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
    })
}

fn graph_bytes_touched() -> (u64, u64) {
    let input_words = GRAPH_NODE_COUNT as usize
        + GRAPH_NODE_COUNT as usize
        + 1
        + GRAPH_EDGE_COUNT as usize
        + GRAPH_EDGE_COUNT as usize
        + GRAPH_NODE_COUNT as usize
        + GRAPH_WORD_COUNT
        + GRAPH_WORD_COUNT;
    ((input_words * 4) as u64, (GRAPH_WORD_COUNT * 4) as u64)
}

fn irregular_graph_fixture(node_count: u32, edge_count: u32) -> DataflowGraphFixture {
    let node_len = node_count as usize;
    let edge_len = edge_count as usize;
    let words = bitset_words(node_count) as usize;
    let nodes = (0..node_count)
        .map(|node| node.rotate_left(3) ^ (node.wrapping_mul(17) & 0xFF))
        .collect::<Vec<_>>();
    let node_tags = (0..node_count)
        .map(|node| {
            if node % 4096 == 0 {
                edge_kind::CALL_ARG
            } else if node % 97 == 0 {
                edge_kind::CONTROL
            } else {
                edge_kind::ASSIGNMENT
            }
        })
        .collect::<Vec<_>>();
    let mut edge_offsets = Vec::with_capacity(node_len.saturating_add(1));
    let mut edge_targets = Vec::with_capacity(edge_len);
    let mut edge_kind_mask = Vec::with_capacity(edge_len);
    let mut remaining_edges = edge_len;

    for src in 0..node_count {
        edge_offsets.push(edge_targets.len() as u32);
        if remaining_edges == 0 {
            continue;
        }
        let degree = if src + 1 == node_count {
            remaining_edges
        } else {
            planned_irregular_out_degree(src).min(remaining_edges)
        };
        for lane in 0..degree {
            edge_targets.push(irregular_edge_target(node_count, src, lane as u32));
            edge_kind_mask.push(irregular_edge_kind(src, lane as u32));
        }
        remaining_edges -= degree;
    }
    edge_offsets.push(edge_targets.len() as u32);

    let mut frontier_in = vec![0_u32; words];
    for node in 0..node_count {
        if node == 0 || node % 4096 == 0 || node % 8191 == 17 {
            set_bit(&mut frontier_in, node);
        }
    }
    let frontier_out_seed = frontier_in.clone();

    DataflowGraphFixture {
        nodes,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_tags,
        frontier_in,
        frontier_out_seed,
    }
}

fn planned_irregular_out_degree(node: u32) -> usize {
    if node % 16_384 == 0 {
        512
    } else if node % 4096 == 0 {
        128
    } else if node % 1024 == 0 {
        64
    } else if node % 257 == 0 {
        16
    } else if node % 31 == 0 {
        4
    } else if node % 7 == 0 {
        2
    } else {
        1
    }
}

fn irregular_edge_target(node_count: u32, src: u32, lane: u32) -> u32 {
    if node_count <= 1 {
        return 0;
    }
    let span = node_count - 1;
    let mixed = src
        .wrapping_mul(1_103_515_245)
        .wrapping_add(lane.wrapping_mul(2_654_435_761))
        .rotate_left((lane & 15) + 1);
    let mut dst = 1 + (mixed % span);
    if dst == src {
        dst = 1 + (dst % span);
    }
    dst
}

fn irregular_edge_kind(src: u32, lane: u32) -> u32 {
    match src.wrapping_mul(31).wrapping_add(lane.wrapping_mul(17)) % 10 {
        0 => edge_kind::ASSIGNMENT,
        1 => edge_kind::CALL_ARG,
        2 => edge_kind::RETURN,
        3 => edge_kind::PHI,
        4 => edge_kind::ALIAS,
        5 => edge_kind::MEM_STORE,
        6 => edge_kind::MEM_LOAD,
        7 => edge_kind::MUT_REF,
        8 => edge_kind::DOMINANCE,
        _ => edge_kind::CONTROL,
    }
}

fn graph_step_baseline_words(
    fixture: &DataflowGraphFixture,
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    csr_step_cpu_ref(
        node_count,
        &fixture.edge_offsets,
        &fixture.edge_targets,
        &fixture.edge_kind_mask,
        &fixture.frontier_in,
        allow_mask,
    )
}

fn set_bit(words: &mut [u32], node: u32) {
    let word = (node / 32) as usize;
    if let Some(slot) = words.get_mut(word) {
        *slot |= 1_u32 << (node % 32);
    }
}

fn bitset_popcount(words: &[u32]) -> u64 {
    words.iter().map(|word| u64::from(word.count_ones())).sum()
}

fn max_out_degree(edge_offsets: &[u32]) -> u32 {
    edge_offsets
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .max()
        .unwrap_or(0)
}

inventory::submit! {
    &DataflowIfdsStep as &'static dyn BenchCase
}

inventory::submit! {
    &DataflowPointsToAliasStep as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataflow_benchmark_ids_are_consumer_neutral() {
        let cases: [(&dyn BenchCase, &str, &str); 3] = [
            (
                &DataflowReachingDefBitset,
                "dataflow.reaching_def.bitset.1m",
                "Dataflow Reaching-Def Bitset 1M",
            ),
            (
                &DataflowIfdsStep,
                "dataflow.ifds.step.262k",
                "Dataflow IFDS Step 262K",
            ),
            (
                &DataflowPointsToAliasStep,
                "dataflow.points_to.alias.step.262k",
                "Dataflow Points-To Alias Step 262K",
            ),
        ];
        for (case, expected_id, expected_name) in cases {
            let metadata = case.metadata();
            assert_eq!(metadata.id.0, expected_id);
            assert_eq!(metadata.name, expected_name);
        }
    }

    #[test]
    fn reaching_def_prepare_caches_encoded_inputs_and_baseline() {
        let prepared = prepare_reaching_def_bitset(None).unwrap();
        assert_eq!(prepared.encoded_inputs.len(), 3);
        assert_eq!(
            prepared.encoded_inputs.iter().map(Vec::len).sum::<usize>(),
            WORD_COUNT * 12
        );
        assert_eq!(prepared.baseline_output.len(), WORD_COUNT * 4);
        let (gen_kill_in, use_set, _) = reaching_def_fixture_words();
        assert_eq!(
            prepared.baseline_output,
            encode_u32_words(&bitset_and_cpu_ref(&gen_kill_in, &use_set))
        );
    }

    #[test]
    fn graph_prepare_caches_encoded_inputs_and_baseline() {
        let prepared = irregular_graph_prepared(
            dataflow_ifds::ifds_reach_step(
                ProgramGraphShape::new(GRAPH_NODE_COUNT, GRAPH_EDGE_COUNT),
                "frontier_in",
                "frontier_out",
            ),
            "ifds_step",
            DATAFLOW_IFDS_ALLOW_MASK,
            None,
        )
        .unwrap();
        assert_eq!(prepared.encoded_inputs.len(), 7);
        assert_eq!(prepared.baseline_output.len(), GRAPH_WORD_COUNT * 4);
        assert_eq!(
            prepared.encoded_inputs.iter().map(Vec::len).sum::<usize>() as u64,
            graph_bytes_touched().0
        );
        assert!(
            prepared.active_sources > 1,
            "dataflow graph fixture must exercise more than one active CSR row"
        );
        assert!(
            prepared.max_out_degree >= 128,
            "dataflow graph fixture must include high-degree rows"
        );
        assert!(
            prepared.allowed_edges < u64::from(GRAPH_EDGE_COUNT),
            "IFDS baseline must exclude non-IFDS edge kinds in the mixed-edge fixture"
        );
    }

    #[test]
    fn irregular_graph_fixture_has_skew_and_mixed_edge_kinds() {
        let fixture = irregular_graph_fixture(8192, 8191);

        assert_eq!(fixture.edge_offsets.len(), 8193);
        assert_eq!(fixture.edge_targets.len(), 8191);
        assert_eq!(fixture.edge_kind_mask.len(), 8191);
        assert!(
            max_out_degree(&fixture.edge_offsets) >= 128,
            "fixture must include hub-like rows instead of a linear chain"
        );
        assert!(
            bitset_popcount(&fixture.frontier_in) > 2,
            "fixture must seed multiple active sources"
        );
        assert!(
            fixture
                .edge_kind_mask
                .iter()
                .any(|&kind| (kind & DATAFLOW_IFDS_ALLOW_MASK) == 0),
            "fixture must include edges that IFDS masks out"
        );
        assert!(
            fixture
                .edge_kind_mask
                .iter()
                .any(|&kind| (kind & DATAFLOW_IFDS_ALLOW_MASK) != 0),
            "fixture must include edges that IFDS traverses"
        );
    }

    #[test]
    fn graph_baseline_uses_analysis_specific_edge_mask() {
        let fixture = irregular_graph_fixture(8192, 8191);
        let ifds = graph_step_baseline_words(&fixture, 8192, DATAFLOW_IFDS_ALLOW_MASK);
        let all_edges = graph_step_baseline_words(&fixture, 8192, u32::MAX);

        assert_ne!(
            ifds, all_edges,
            "mixed-edge dataflow baseline must distinguish IFDS from points-to traversal"
        );
    }
}

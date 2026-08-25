use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{digest64_buffers, BenchMetrics, MetricPoint};
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, ResidentInputPool, ResidentInputSet,
    TransferAccounting,
};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::graph::program_graph::ProgramGraphShape;

const STRING_BITMAP_RESIDENT_BATCH_SIZE: usize = 16;
const METADATA_CONDITION_RESIDENT_BATCH_SIZE: usize = 16;

pub struct SparseOutputCompactionCount;
pub struct CallgraphReachabilityStep;
pub struct MetadataConditionBatch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseMacroFamily {
    Scan,
    Flow,
    Graph,
    Parser,
    Egraph,
    Resident,
    Matrix,
    Condition,
}

struct MetadataConditionPrepared {
    program: Program,
    filesize: Vec<u32>,
    header: Vec<u32>,
    entropy: Vec<u32>,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    expected_count: u32,
    resident: Option<ResidentInputSet>,
    resident_batch: Option<ResidentInputPool>,
}

struct SyntheticCountWorkload {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    tags: &'static [&'static str],
    owner_crate: &'static str,
    primitive: &'static str,
    baseline: &'static str,
    metric_name: &'static str,
    family: ReleaseMacroFamily,
    records: u32,
    min_speedup_x: f64,
    pattern: SyntheticPattern,
}

struct SyntheticCountPrepared {
    program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    logical_output_bytes: u64,
    output_reset_payload: Vec<u8>,
    baseline: SyntheticBaseline,
    resident: Option<ResidentInputSet>,
    resident_batch: Option<ResidentInputPool>,
}

enum SyntheticBaseline {
    Count {
        expected: u32,
    },
    StringBitmap {
        pattern_bitmap: Vec<u32>,
        rule_bitmap: Vec<u32>,
    },
}

/// Public release macro workload program descriptor for local benchmark entrypoints.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ReleaseMacroProgramSpec {
    /// Stable benchmark case id.
    pub id: &'static str,
    /// Human-readable benchmark name.
    pub name: &'static str,
    /// Logical records processed by the release workload.
    pub records: u32,
    /// Number of input buffers in the generated release workload.
    pub input_buffers: usize,
    /// Minimum CUDA speedup contract attached to this release workload.
    pub min_speedup_x: u32,
    /// Typed release workload family.
    pub family: ReleaseMacroFamily,
    /// Owner crate responsible for this workload.
    pub owner_crate: &'static str,
}

/// Generated release workload case with concrete inputs and CPU-oracle outputs.
#[derive(Clone)]
pub struct ReleaseMacroGeneratedCase {
    /// Public descriptor for the generated workload shape.
    pub spec: ReleaseMacroProgramSpec,
    /// IR program generated for this workload shape.
    pub program: Program,
    /// Concrete input byte buffers.
    pub inputs: Vec<Vec<u8>>,
    /// Expected output byte buffers from the CPU oracle.
    pub expected_outputs: Vec<Vec<u8>>,
    /// Total logical input bytes for the generated inputs.
    pub input_bytes_total: u64,
    /// Digest of expected output bytes.
    pub expected_output_digest: u64,
}

/// Runtime evidence required for held-out math and NN release kernels.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReleaseMathNnKernelEvidence {
    /// Stable benchmark case id.
    pub case_id: &'static str,
    /// CPU oracle output digest.
    pub cpu_digest: u64,
    /// GPU/backend output digest.
    pub gpu_digest: u64,
    /// Absolute tolerance scaled by 1e9.
    pub tolerance_abs_e9: u64,
    /// Active kernel/device time in nanoseconds, or wall time when unavailable.
    pub active_time_ns: u64,
    /// Accounted transfer bytes for this run.
    pub transfer_bytes: u64,
    /// Selected planner/kernel path.
    pub selected_kernel_path: &'static str,
}

/// Validate held-out math and NN release-kernel evidence.
///
/// # Errors
/// Returns `Err` when digests, tolerance, active time, transfer accounting, or
/// planner path evidence is missing.
pub fn validate_release_math_nn_kernel_evidence(
    evidence: &ReleaseMathNnKernelEvidence,
) -> Result<(), String> {
    if evidence.cpu_digest == 0 {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing cpu_digest.",
            evidence.case_id
        ));
    }
    if evidence.gpu_digest == 0 {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing gpu_digest.",
            evidence.case_id
        ));
    }
    if evidence.tolerance_abs_e9 == 0 {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing tolerance_abs_e9.",
            evidence.case_id
        ));
    }
    if evidence.active_time_ns == 0 {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing active_time_ns.",
            evidence.case_id
        ));
    }
    if evidence.transfer_bytes == 0 {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing transfer_bytes.",
            evidence.case_id
        ));
    }
    if evidence.selected_kernel_path.is_empty() {
        return Err(format!(
            "Fix: release math/NN evidence `{}` is missing selected_kernel_path.",
            evidence.case_id
        ));
    }
    Ok(())
}

/// Scan competitor corpus metadata required for release workload evidence.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReleaseScanCompetitorCorpusMetadata {
    /// Stable benchmark case id.
    pub case_id: &'static str,
    /// Rule family, for example literal, regex, mixed, or secret-token.
    pub rule_family: &'static str,
    /// Number of patterns in the rule set.
    pub pattern_count: u32,
    /// Literal density in basis points across the pattern set.
    pub literal_density_bps: u16,
    /// Construct classes represented by this scan corpus.
    pub construct_classes: &'static [&'static str],
    /// Haystack corpus id.
    pub haystack_corpus_id: &'static str,
    /// Competitor baseline engine.
    pub baseline_engine: &'static str,
    /// Exact semantic exclusions for unsupported competitor constructs.
    pub unsupported_construct_reasons: &'static [&'static str],
}

/// Validate scan competitor corpus metadata.
///
/// # Errors
/// Returns `Err` when required corpus, pattern, construct, baseline, or
/// unsupported-construct fields are missing or malformed.
pub fn validate_release_scan_competitor_corpus_metadata(
    metadata: &ReleaseScanCompetitorCorpusMetadata,
) -> Result<(), String> {
    if metadata.case_id.is_empty() {
        return Err("Fix: scan competitor metadata case_id must be non-empty.".to_string());
    }
    if metadata.rule_family.is_empty() {
        return Err(format!(
            "Fix: scan competitor metadata `{}` is missing rule_family.",
            metadata.case_id
        ));
    }
    if metadata.pattern_count == 0 {
        return Err(format!(
            "Fix: scan competitor metadata `{}` must record a positive pattern_count.",
            metadata.case_id
        ));
    }
    if metadata.literal_density_bps > 10_000 {
        return Err(format!(
            "Fix: scan competitor metadata `{}` literal_density_bps must be <= 10000.",
            metadata.case_id
        ));
    }
    if metadata.construct_classes.is_empty()
        || metadata
            .construct_classes
            .iter()
            .any(|construct| construct.is_empty())
    {
        return Err(format!(
            "Fix: scan competitor metadata `{}` must record non-empty construct_classes.",
            metadata.case_id
        ));
    }
    if metadata.haystack_corpus_id.is_empty() {
        return Err(format!(
            "Fix: scan competitor metadata `{}` is missing haystack_corpus_id.",
            metadata.case_id
        ));
    }
    if metadata.baseline_engine.is_empty() {
        return Err(format!(
            "Fix: scan competitor metadata `{}` is missing baseline_engine.",
            metadata.case_id
        ));
    }
    for reason in metadata.unsupported_construct_reasons {
        if reason.is_empty() || !reason.contains("Fix:") {
            return Err(format!(
                "Fix: scan competitor metadata `{}` unsupported construct reasons must be exact and actionable.",
                metadata.case_id
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SyntheticPattern {
    ConditionEval,
    StringBitmapScatter,
    OffsetCountAggregation,
    EntropyWindow,
    QuantifiedLoops,
    AliasReachingDef,
    IfdsWitness,
    CAstTraversal,
    MegakernelQueuedBatch,
    EgraphSaturation,
}

#[path = "release_workloads/families.rs"]
mod release_workload_families;
use release_workload_families::{
    release_macro_workloads, release_macro_workloads_for_family, ALIAS_REACHING_DEF,
    C_AST_TRAVERSAL, CONDITION_EVAL_BATCH, EGRAPH_SATURATION, ENTROPY_WINDOW, IFDS_WITNESS,
    MEGAKERNEL_QUEUE,
    OFFSET_COUNT_AGGREGATION, QUANTIFIED_LOOPS, STRING_BITMAP_SCATTER,
};

const RELEASE_SUITES: &[crate::api::suite::SuiteKind] = &[
    crate::api::suite::SuiteKind::Release,
    crate::api::suite::SuiteKind::Gpu,
    crate::api::suite::SuiteKind::Deep,
    crate::api::suite::SuiteKind::Honest,
];

const SPARSE_ITEMS: u32 = 1_048_576;
const METADATA_RECORDS: u32 = 1_048_576;
const METADATA_OUTPUT_RESET_BYTES: u64 = 4;
const CALLGRAPH_NODES: u32 = 262_144;
const CALLGRAPH_EDGES: u32 = CALLGRAPH_NODES - 1;
const CALLGRAPH_WORDS: usize = CALLGRAPH_NODES.div_ceil(32) as usize;

impl BenchCase for SparseOutputCompactionCount {
    fn id(&self) -> BenchId {
        BenchId("sparse.compaction.count.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Sparse Output Compaction Count 1M".to_string(),
            description:
                "Sparse hit counting front-end for GPU output compaction over a 1M candidate stream"
                    .to_string(),
            tags: vec![
                "sparse".to_string(),
                "compaction".to_string(),
                "compact".to_string(),
                "append".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Runtime,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-runtime".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        gpu_requirements((SPARSE_ITEMS as u64 + 1) * 4)
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_100x(
            "sparse output compaction count",
            "vyre-runtime",
            "optimized CPU fired-rule collection over predicate masks",
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(sparse_output_compaction_count_program()))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let program = crate::api::case::prepared_program(prepared)?;
        let mut flags = Vec::with_capacity(SPARSE_ITEMS as usize);
        let mut expected = 0u32;
        for index in 0..SPARSE_ITEMS {
            let hit = sparse_compaction_flag(index) != 0;
            expected += u32::from(hit);
            flags.push(u32::from(hit));
        }
        let inputs = vec![vec![0; 4], encode_u32_words(&flags)];
        let timed = ctx
            .dispatch_timed(program, &inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let baseline_start = std::time::Instant::now();
        let mut fired_rules = Vec::new();
        for index in 0..SPARSE_ITEMS {
            if sparse_compaction_flag(index) != 0 {
                fired_rules.push(index);
            }
        }
        let cpu_count = fired_rules.len() as u32;
        let baseline_wall = baseline_start.elapsed().as_nanos() as u64;
        if cpu_count != expected {
            return Err(BenchError::CorrectnessViolation(
                "sparse CPU baseline count disagreed with generator expectation".to_string(),
            ));
        }
        let baseline_outputs = vec![cpu_count.to_le_bytes().to_vec()];
        bench_run_from_timed(
            timed,
            inputs,
            baseline_outputs,
            baseline_wall,
            "sparse_items",
            SPARSE_ITEMS,
        )
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn sparse_output_compaction_count_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read_write("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("flags", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(SPARSE_ITEMS),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::and(
                    Expr::lt(Expr::var("idx"), Expr::u32(SPARSE_ITEMS)),
                    Expr::ne(Expr::load("flags", Expr::var("idx")), Expr::u32(0)),
                ),
                vec![Node::let_bind(
                    "_slot",
                    Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                )],
            ),
        ],
    )
}

fn sparse_compaction_flag(index: u32) -> u32 {
    let mut hash = index ^ 0x9E37_79B9;
    for lane in 0..18 {
        hash = hash
            .rotate_left(5)
            .wrapping_mul(0x85EB_CA6B)
            .wrapping_add(0xC2B2_AE35 ^ lane);
    }
    u32::from(index % 97 == 0 || index % 4099 == 17 || hash == 0)
}

impl BenchCase for CallgraphReachabilityStep {
    fn id(&self) -> BenchId {
        BenchId("callgraph.reachability.step.262k".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Callgraph Reachability Step 262K".to_string(),
            description: "Graph reachability step over a callgraph-shaped CSR workload".to_string(),
            tags: vec![
                "callgraph".to_string(),
                "reachability".to_string(),
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
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        gpu_requirements(graph_input_bytes().saturating_add((CALLGRAPH_WORDS * 4) as u64))
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "callgraph reachability CSR step",
            "vyre-primitives",
            "optimized CPU graph reachability and witness extraction",
            25.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let shape = ProgramGraphShape::new(CALLGRAPH_NODES, CALLGRAPH_EDGES);
        Ok(Box::new(
            vyre_primitives::graph::csr_forward_traverse::csr_forward_traverse(
                shape,
                "frontier_in",
                "frontier_out",
                1,
            ),
        ))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let program = crate::api::case::prepared_program(prepared)?;
        let graph = linear_graph_inputs();
        let timed = ctx
            .dispatch_timed(program, &graph.inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let baseline_start = std::time::Instant::now();
        let mut expected = release_benchmark_csr_forward_baseline(
            CALLGRAPH_NODES,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            &graph.frontier_in,
            1,
        );
        let witness_digest = callgraph_witness_digest(
            CALLGRAPH_NODES,
            &graph.edge_offsets,
            &graph.edge_targets,
            &graph.edge_kind_mask,
            &graph.frontier_in,
            1,
        );
        for (out, seed) in expected.iter_mut().zip(graph.frontier_out_seed.iter()) {
            *out |= *seed;
        }
        let baseline_outputs = vec![encode_u32_words(&expected)];
        let baseline_wall = baseline_start.elapsed().as_nanos() as u64;
        let mut run = bench_run_from_timed(
            timed,
            graph.inputs,
            baseline_outputs,
            baseline_wall,
            "callgraph_nodes",
            CALLGRAPH_NODES,
        )?;
        run.metrics.custom.push(MetricPoint {
            name: "callgraph_witness_digest".to_string(),
            value: u64::from(witness_digest),
        });
        Ok(run)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

impl BenchCase for MetadataConditionBatch {
    fn id(&self) -> BenchId {
        BenchId("metadata.condition.filesize_header.1m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Metadata Condition File/Header 1M".to_string(),
            description: "File metadata and PE/header-style condition evaluation over 1M records"
                .to_string(),
            tags: vec![
                "metadata".to_string(),
                "condition".to_string(),
                "filesize".to_string(),
                "header".to_string(),
                "pe".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-libs".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        gpu_requirements((METADATA_RECORDS as u64 * 12) + 4)
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "metadata condition evaluation",
            "vyre-libs",
            "optimized CPU PE-header predicate evaluator",
            50.0,
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = Program::wrapped(
            vec![
                BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
                BufferDecl::storage("filesize", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(METADATA_RECORDS),
                BufferDecl::storage("header", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(METADATA_RECORDS),
                BufferDecl::storage("entropy_x1000", 3, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(METADATA_RECORDS),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("idx"), Expr::u32(METADATA_RECORDS)),
                        Expr::and(
                            Expr::gt(Expr::load("filesize", Expr::var("idx")), Expr::u32(4096)),
                            Expr::and(
                                Expr::eq(
                                    Expr::load("header", Expr::var("idx")),
                                    Expr::u32(0x0000_4550),
                                ),
                                Expr::gt(
                                    Expr::load("entropy_x1000", Expr::var("idx")),
                                    Expr::u32(7200),
                                ),
                            ),
                        ),
                    ),
                    vec![Node::let_bind(
                        "_slot",
                        Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        );
        let mut filesize = Vec::with_capacity(METADATA_RECORDS as usize);
        let mut header = Vec::with_capacity(METADATA_RECORDS as usize);
        let mut entropy = Vec::with_capacity(METADATA_RECORDS as usize);
        let mut expected = 0u32;
        for index in 0..METADATA_RECORDS {
            let size = 1024 + (index.wrapping_mul(13) % 131_072);
            let hdr = if index % 5 == 0 {
                0x0000_4550
            } else {
                0x464C_457F
            };
            let ent = 5000 + (index.wrapping_mul(17) % 4500);
            expected += u32::from(size > 4096 && hdr == 0x0000_4550 && ent > 7200);
            filesize.push(size);
            header.push(hdr);
            entropy.push(ent);
        }
        let inputs = vec![
            encode_u32_words(&filesize),
            encode_u32_words(&header),
            encode_u32_words(&entropy),
        ];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_program_ordered_with_zeroed_outputs_optional(
            ctx,
            &program,
            &inputs,
            "metadata condition bench",
        )?;
        let resident_batch =
            ResidentInputPool::upload_program_ordered_with_zeroed_outputs_optional(
                ctx,
                &program,
                &inputs,
                METADATA_CONDITION_RESIDENT_BATCH_SIZE,
                "metadata condition bench batch",
            )?;
        Ok(Box::new(MetadataConditionPrepared {
            program,
            filesize,
            header,
            entropy,
            inputs,
            input_bytes_total,
            expected_count: expected,
            resident,
            resident_batch,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<MetadataConditionPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<MetadataConditionPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "metadata condition prepared payload type mismatch".to_string(),
                )
            })?;
        let mut metadata_batch_wall_ns = None;
        let mut metadata_batch_len = None;
        let (timed, resident_used, resident_reset_bytes) = if let (
            Some(resident_batch),
            Some(pipeline),
        ) = (
            prepared.resident_batch.as_ref(),
            ctx.compiled_pipeline_for(&prepared.program)
                .map_err(|error| BenchError::BackendFailed(error.to_string()))?,
        ) {
            resident_batch.upload_resource_to_all_sets(
                0,
                &0u32.to_le_bytes(),
                "metadata condition resident batch counter reset",
            )?;
            let config = crate::api::case::dispatch_config_with_inferred_grid(
                &prepared.program,
                &prepared.inputs,
                &ctx.dispatch_config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
            vyre_driver::validate_program_for_backend(
                ctx.preferred_backend.as_ref(),
                &prepared.program,
                &config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
            match resident_batch.dispatch_compiled_batch_timed(
                pipeline,
                METADATA_CONDITION_RESIDENT_BATCH_SIZE,
                &config,
            ) {
                Ok(batch) => {
                    if batch.outputs.len() != METADATA_CONDITION_RESIDENT_BATCH_SIZE {
                        return Err(BenchError::ExecutionFailed(format!(
                            "metadata condition resident batch returned {} output row(s), expected {}",
                            batch.outputs.len(),
                            METADATA_CONDITION_RESIDENT_BATCH_SIZE
                        )));
                    }
                    let first_outputs = batch.outputs.first().cloned().ok_or_else(|| {
                        BenchError::ExecutionFailed(
                            "metadata condition resident batch returned no output rows".to_string(),
                        )
                    })?;
                    if let Some((index, _)) = batch
                        .outputs
                        .iter()
                        .enumerate()
                        .find(|(_, outputs)| **outputs != first_outputs)
                    {
                        return Err(BenchError::CorrectnessViolation(format!(
                            "metadata condition resident batch output row {index} disagreed with row 0"
                        )));
                    }
                    metadata_batch_wall_ns = Some(batch.wall_ns_total);
                    metadata_batch_len = Some(batch.batch_len as u64);
                    (
                        vyre_driver::TimedDispatchResult {
                            outputs: first_outputs,
                            wall_ns: batch.per_item_wall_ns(),
                            device_ns: None,
                            enqueue_ns: None,
                            wait_ns: None,
                        },
                        true,
                        METADATA_OUTPUT_RESET_BYTES,
                    )
                }
                Err(vyre_driver::BackendError::UnsupportedFeature { .. }) => {
                    dispatch_single_metadata_resident(ctx, prepared)?
                }
                Err(error) => return Err(BenchError::BackendFailed(error.to_string())),
            }
        } else {
            dispatch_single_metadata_resident(ctx, prepared)?
        };
        let outputs = timed.outputs.clone();
        let baseline_start = std::time::Instant::now();
        let mut cpu_count = 0u32;
        for index in 0..prepared.filesize.len() {
            cpu_count += u32::from(
                prepared.filesize[index] > 4096
                    && prepared.header[index] == 0x0000_4550
                    && prepared.entropy[index] > 7200,
            );
        }
        let baseline_wall = baseline_start.elapsed().as_nanos() as u64;
        if cpu_count != prepared.expected_count {
            return Err(BenchError::CorrectnessViolation(
                "metadata CPU baseline count disagreed with generator expectation".to_string(),
            ));
        }
        let baseline_outputs = vec![cpu_count.to_le_bytes().to_vec()];
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = resident_reset_transfer_accounting(
            prepared.input_bytes_total,
            output_bytes,
            resident_used,
            resident_reset_bytes,
        );
        let logical_bytes_touched = prepared.input_bytes_total.saturating_add(output_bytes);
        let mut custom = vec![
            MetricPoint {
                name: "metadata_records".to_string(),
                value: u64::from(METADATA_RECORDS),
            },
            MetricPoint {
                name: "metadata_expected_matches".to_string(),
                value: u64::from(prepared.expected_count),
            },
            MetricPoint {
                name: "metadata_resident_buffers".to_string(),
                value: u64::from(resident_used),
            },
            MetricPoint {
                name: "metadata_resident_reset_bytes".to_string(),
                value: resident_reset_bytes,
            },
        ];
        if let Some(batch_wall_ns) = metadata_batch_wall_ns {
            custom.push(MetricPoint {
                name: "metadata_resident_batch_wall_ns".to_string(),
                value: batch_wall_ns,
            });
        }
        if let Some(batch_len) = metadata_batch_len {
            custom.push(MetricPoint {
                name: "metadata_resident_batch_len".to_string(),
                value: batch_len,
            });
        }
        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(logical_bytes_touched),
                custom,
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_wall),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                bytes_touched:
                    Some(
                        prepared.input_bytes_total.saturating_add(
                            baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64,
                        ),
                    ),
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: Some(baseline_outputs),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<MetadataConditionPrepared>()
            .map(|prepared| (prepared.input_bytes_total, METADATA_OUTPUT_RESET_BYTES))
            .unwrap_or((
                u64::from(METADATA_RECORDS) * 12,
                METADATA_OUTPUT_RESET_BYTES,
            ))
    }
}

fn resident_reset_transfer_accounting(
    input_bytes_total: u64,
    output_bytes_total: u64,
    resident_used: bool,
    resident_reset_bytes: u64,
) -> TransferAccounting {
    let bytes_read = if resident_used {
        resident_reset_bytes
    } else {
        input_bytes_total
    };
    TransferAccounting {
        bytes_touched: bytes_read.saturating_add(output_bytes_total),
        bytes_read,
        bytes_written: output_bytes_total,
    }
}

fn dispatch_single_metadata_resident(
    ctx: &BenchContext,
    prepared: &MetadataConditionPrepared,
) -> Result<(vyre_driver::TimedDispatchResult, bool, u64), BenchError> {
    let resident_reset_bytes = if let Some(resident) = prepared.resident.as_ref() {
        resident.upload_resource(
            0,
            &0u32.to_le_bytes(),
            "metadata condition resident counter reset",
        )?;
        METADATA_OUTPUT_RESET_BYTES
    } else {
        0
    };
    let dispatch = dispatch_program_timed(
        ctx,
        &prepared.program,
        prepared.resident.as_ref(),
        &prepared.inputs,
        &ctx.dispatch_config,
    )?;
    Ok((dispatch.timed, dispatch.resident_used, resident_reset_bytes))
}

fn dispatch_single_synthetic_resident(
    ctx: &BenchContext,
    prepared: &SyntheticCountPrepared,
) -> Result<
    (
        vyre_driver::TimedDispatchResult,
        bool,
        u64,
        Option<u64>,
        Option<u64>,
    ),
    BenchError,
> {
    if let Some(resident) = prepared.resident.as_ref() {
        if !prepared.output_reset_payload.is_empty() {
            resident.upload_resource(
                0,
                &prepared.output_reset_payload,
                "synthetic release resident output reset",
            )?;
        }
        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            Some(resident),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        return Ok((
            dispatch.timed,
            dispatch.resident_used,
            prepared.output_reset_payload.len() as u64,
            None,
            None,
        ));
    }

    let dispatch = dispatch_program_timed(
        ctx,
        &prepared.program,
        None,
        &prepared.inputs,
        &ctx.dispatch_config,
    )?;
    Ok((dispatch.timed, dispatch.resident_used, 0, None, None))
}

impl BenchCase for SyntheticCountWorkload {
    fn id(&self) -> BenchId {
        BenchId(self.id.to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        let mut tags = self
            .tags
            .iter()
            .map(|tag| (*tag).to_string())
            .collect::<Vec<_>>();
        tags.push("release".to_string());
        BenchMetadata {
            id: self.id(),
            name: self.name.to_string(),
            description: self.description.to_string(),
            tags,
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: self.owner_crate.to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        RELEASE_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        gpu_requirements((self.records as u64 * pattern_input_count(self.pattern) as u64 * 4) + 4)
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            self.primitive,
            self.owner_crate,
            self.baseline,
            self.min_speedup_x,
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let program = build_synthetic_release_program(self.pattern, self.records);
        let (inputs, baseline) = match self.pattern {
            SyntheticPattern::StringBitmapScatter => {
                let generated = string_bitmap_scatter_inputs(self.records);
                (
                    generated.inputs,
                    SyntheticBaseline::StringBitmap {
                        pattern_bitmap: generated.pattern_bitmap,
                        rule_bitmap: generated.rule_bitmap,
                    },
                )
            }
            pattern => {
                let generated = synthetic_inputs(pattern, self.records);
                (
                    generated.inputs,
                    SyntheticBaseline::Count {
                        expected: generated.expected,
                    },
                )
            }
        };
        let input_bytes_total = input_bytes_total(&inputs);
        let logical_output_bytes = synthetic_logical_output_bytes(self.pattern, self.records);
        let output_reset_payload =
            vec![0u8; synthetic_output_reset_bytes(self.pattern, self.records)];
        let resident = ResidentInputSet::upload_program_ordered_with_zeroed_outputs_optional(
            ctx,
            &program,
            &inputs,
            "synthetic release workload",
        )?;
        let resident_batch = if self.pattern == SyntheticPattern::StringBitmapScatter {
            ResidentInputPool::upload_program_ordered_with_zeroed_outputs_optional(
                ctx,
                &program,
                &inputs,
                STRING_BITMAP_RESIDENT_BATCH_SIZE,
                "synthetic release workload batch",
            )?
        } else {
            None
        };
        Ok(Box::new(SyntheticCountPrepared {
            program,
            inputs,
            input_bytes_total,
            logical_output_bytes,
            output_reset_payload,
            baseline,
            resident,
            resident_batch,
        }))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<SyntheticCountPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "synthetic release prepared payload type mismatch".to_string(),
                )
            })?;
        let (timed, resident_used, resident_reset_bytes, batch_wall_ns, batch_len) = if let (
            SyntheticPattern::StringBitmapScatter,
            true,
            Some(resident_batch),
            Some(pipeline),
        ) = (
            self.pattern,
            ctx.preferred_backend.id() == "cuda",
            prepared.resident_batch.as_ref(),
            ctx.compiled_pipeline_for(&prepared.program)
                .map_err(|error| BenchError::BackendFailed(error.to_string()))?,
        ) {
            let config = crate::api::case::dispatch_config_with_inferred_grid(
                &prepared.program,
                &prepared.inputs,
                &ctx.dispatch_config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
            vyre_driver::validate_program_for_backend(
                ctx.preferred_backend.as_ref(),
                &prepared.program,
                &config,
            )
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
            match resident_batch.dispatch_compiled_batch_timed(
                pipeline,
                STRING_BITMAP_RESIDENT_BATCH_SIZE,
                &config,
            ) {
                Ok(batch) => {
                    if batch.outputs.len() != STRING_BITMAP_RESIDENT_BATCH_SIZE {
                        return Err(BenchError::ExecutionFailed(format!(
                            "{} resident batch returned {} output row(s), expected {}",
                            self.id,
                            batch.outputs.len(),
                            STRING_BITMAP_RESIDENT_BATCH_SIZE
                        )));
                    }
                    let first_outputs = batch.outputs.first().cloned().ok_or_else(|| {
                        BenchError::ExecutionFailed(format!(
                            "{} resident batch returned no output rows",
                            self.id
                        ))
                    })?;
                    if let Some((index, _)) = batch
                        .outputs
                        .iter()
                        .enumerate()
                        .find(|(_, outputs)| **outputs != first_outputs)
                    {
                        return Err(BenchError::CorrectnessViolation(format!(
                            "{} resident batch output row {index} disagreed with row 0",
                            self.id
                        )));
                    }
                    (
                        vyre_driver::TimedDispatchResult {
                            outputs: first_outputs,
                            wall_ns: batch.per_item_wall_ns(),
                            device_ns: None,
                            enqueue_ns: None,
                            wait_ns: None,
                        },
                        true,
                        0,
                        Some(batch.wall_ns_total),
                        Some(batch.batch_len as u64),
                    )
                }
                Err(vyre_driver::BackendError::UnsupportedFeature { .. }) => {
                    dispatch_single_synthetic_resident(ctx, prepared)?
                }
                Err(error) => return Err(BenchError::BackendFailed(error.to_string())),
            }
        } else if let Some(resident) = prepared.resident.as_ref() {
            if !prepared.output_reset_payload.is_empty() {
                resident.upload_resource(
                    0,
                    &prepared.output_reset_payload,
                    "synthetic release resident output reset",
                )?;
            }
            let dispatch = dispatch_program_timed(
                ctx,
                &prepared.program,
                Some(resident),
                &prepared.inputs,
                &ctx.dispatch_config,
            )?;
            (
                dispatch.timed,
                dispatch.resident_used,
                prepared.output_reset_payload.len() as u64,
                None,
                None,
            )
        } else {
            let dispatch = dispatch_program_timed(
                ctx,
                &prepared.program,
                None,
                &prepared.inputs,
                &ctx.dispatch_config,
            )?;
            (dispatch.timed, dispatch.resident_used, 0, None, None)
        };
        let baseline_start = std::time::Instant::now();
        let baseline_outputs = match &prepared.baseline {
            SyntheticBaseline::Count { expected } => {
                let cpu_count = synthetic_cpu_count(self.pattern, self.records);
                if cpu_count != *expected {
                    return Err(BenchError::CorrectnessViolation(format!(
                        "{} CPU baseline count disagreed with generator expectation",
                        self.id
                    )));
                }
                vec![cpu_count.to_le_bytes().to_vec()]
            }
            SyntheticBaseline::StringBitmap {
                pattern_bitmap,
                rule_bitmap,
            } => {
                let baseline_words =
                    string_bitmap_scatter_expected_words(pattern_bitmap, rule_bitmap, self.records);
                vec![encode_u32_words(&baseline_words)]
            }
        };
        let baseline_wall = baseline_start.elapsed().as_nanos() as u64;
        let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = resident_reset_transfer_accounting(
            prepared.input_bytes_total,
            output_bytes,
            resident_used,
            resident_reset_bytes,
        );
        let logical_bytes_touched = prepared
            .input_bytes_total
            .saturating_add(prepared.logical_output_bytes);
        let mut run = bench_run_from_timed_with_accounting(
            timed,
            prepared.input_bytes_total,
            baseline_outputs,
            baseline_wall,
            self.metric_name,
            self.records,
            logical_bytes_touched,
            accounting,
        )?;
        run.metrics.custom.push(MetricPoint {
            name: "synthetic_resident_buffers".to_string(),
            value: u64::from(resident_used),
        });
        run.metrics.custom.push(MetricPoint {
            name: "synthetic_resident_reset_bytes".to_string(),
            value: resident_reset_bytes,
        });
        if let Some(batch_wall_ns) = batch_wall_ns {
            run.metrics.custom.push(MetricPoint {
                name: "synthetic_resident_batch_wall_ns".to_string(),
                value: batch_wall_ns,
            });
        }
        if let Some(batch_len) = batch_len {
            run.metrics.custom.push(MetricPoint {
                name: "synthetic_resident_batch_len".to_string(),
                value: batch_len,
            });
        }
        match &prepared.baseline {
            SyntheticBaseline::Count { expected } => {
                add_release_alias_metrics(self.pattern, self.records, *expected, &mut run);
            }
            SyntheticBaseline::StringBitmap { .. } => {
                run.metrics.custom.push(MetricPoint {
                    name: "scatter_materialized_words".to_string(),
                    value: u64::from(self.records),
                });
            }
        }
        Ok(run)
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<SyntheticCountPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<SyntheticCountPrepared>()
            .map(|prepared| (prepared.input_bytes_total, prepared.logical_output_bytes))
            .unwrap_or((
                self.records as u64 * pattern_input_count(self.pattern) as u64 * 4,
                synthetic_logical_output_bytes(self.pattern, self.records),
            ))
    }
}

fn build_synthetic_release_program(pattern: SyntheticPattern, records: u32) -> Program {
    match pattern {
        SyntheticPattern::ConditionEval => condition_eval_program(records),
        SyntheticPattern::StringBitmapScatter => string_bitmap_scatter_program(records),
        SyntheticPattern::OffsetCountAggregation => offset_count_aggregation_program(records),
        SyntheticPattern::EntropyWindow => entropy_window_program(records),
        SyntheticPattern::QuantifiedLoops => quantified_condition_loops_program(records),
        SyntheticPattern::AliasReachingDef => alias_reaching_def_program(records),
        SyntheticPattern::IfdsWitness => ifds_witness_program(records),
        SyntheticPattern::CAstTraversal => c_ast_traversal_program(records),
        SyntheticPattern::MegakernelQueuedBatch => megakernel_queue_program(records),
        SyntheticPattern::EgraphSaturation => egraph_saturation_program(records),
    }
}

fn gpu_requirements(input_bytes: u64) -> BenchRequirements {
    BenchRequirements {
        needs_gpu: true,
        needs_network: false,
        min_vram_bytes: None,
        min_input_bytes: Some(input_bytes),
        feature_set: vec!["release-workload".to_string()],
    }
}

struct SyntheticInputs {
    inputs: Vec<Vec<u8>>,
    expected: u32,
}

struct StringBitmapScatterInputs {
    inputs: Vec<Vec<u8>>,
    pattern_bitmap: Vec<u32>,
    rule_bitmap: Vec<u32>,
}

fn string_bitmap_scatter_inputs(records: u32) -> StringBitmapScatterInputs {
    let output_words = records.div_ceil(32) as usize;
    let mut pattern_bitmap = Vec::with_capacity(records as usize);
    let mut rule_bitmap = Vec::with_capacity(records as usize);
    for index in 0..records {
        let row = synthetic_row(SyntheticPattern::StringBitmapScatter, index);
        pattern_bitmap.push(row[0]);
        rule_bitmap.push(row[1]);
    }
    let inputs = vec![
        vec![0u8; output_words * 4],
        encode_u32_words(&pattern_bitmap),
        encode_u32_words(&rule_bitmap),
    ];
    StringBitmapScatterInputs {
        inputs,
        pattern_bitmap,
        rule_bitmap,
    }
}

fn string_bitmap_scatter_expected_words(
    pattern_bitmap: &[u32],
    rule_bitmap: &[u32],
    records: u32,
) -> Vec<u32> {
    let mut expected_words = vec![0u32; records.div_ceil(32) as usize];
    for index in 0..records {
        if pattern_bitmap[index as usize] != 0 && rule_bitmap[index as usize] != 0 {
            expected_words[(index / 32) as usize] |= 1u32 << (index & 31);
        }
    }
    expected_words
}

fn synthetic_count_program(pattern: SyntheticPattern, records: u32) -> Program {
    let mut buffers = vec![BufferDecl::output("out_count", 0, DataType::U32).with_count(1)];
    for (binding, name) in pattern_buffers(pattern).iter().enumerate() {
        buffers.push(
            BufferDecl::storage(
                *name,
                (binding + 1) as u32,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(records),
        );
    }
    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::and(
                    Expr::lt(Expr::var("idx"), Expr::u32(records)),
                    pattern_condition(pattern),
                ),
                vec![Node::let_bind(
                    "_slot",
                    Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                )],
            ),
        ],
    )
}

fn condition_eval_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("match_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("rule_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("metadata_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("match_word", load_u32("match_mask")),
                    Node::let_bind("rule_word", load_u32("rule_mask")),
                    Node::let_bind("metadata_word", load_u32("metadata_mask")),
                    Node::let_bind("condition_hits", Expr::u32(0)),
                    Node::loop_for(
                        "condition_lane",
                        Expr::u32(0),
                        Expr::u32(CONDITION_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("match_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("condition_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("rule_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("condition_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("metadata_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("condition_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "condition_hits",
                                Expr::add(Expr::var("condition_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("condition_hits"), Expr::u32(CONDITION_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn string_bitmap_scatter_program(records: u32) -> Program {
    let output_words = records.div_ceil(32);
    Program::wrapped(
        vec![
            BufferDecl::storage("out_flags", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(output_words)
                .with_output_byte_range(0..(output_words as usize * 4)),
            BufferDecl::storage("pattern_bitmap", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("rule_bitmap", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("word_idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("word_idx"), Expr::u32(output_words)),
                vec![
                    Node::let_bind("word", Expr::u32(0)),
                    Node::loop_for(
                        "lane",
                        Expr::u32(0),
                        Expr::u32(32),
                        vec![
                            Node::let_bind(
                                "record_idx",
                                Expr::add(
                                    Expr::shl(Expr::var("word_idx"), Expr::u32(5)),
                                    Expr::var("lane"),
                                ),
                            ),
                            Node::if_then(
                                Expr::and(
                                    Expr::lt(Expr::var("record_idx"), Expr::u32(records)),
                                    Expr::and(
                                        Expr::ne(
                                            Expr::load("pattern_bitmap", Expr::var("record_idx")),
                                            Expr::u32(0),
                                        ),
                                        Expr::ne(
                                            Expr::load("rule_bitmap", Expr::var("record_idx")),
                                            Expr::u32(0),
                                        ),
                                    ),
                                ),
                                vec![Node::assign(
                                    "word",
                                    Expr::bitor(
                                        Expr::var("word"),
                                        Expr::shl(Expr::u32(1), Expr::var("lane")),
                                    ),
                                )],
                            ),
                        ],
                    ),
                    Node::store("out_flags", Expr::var("word_idx"), Expr::var("word")),
                ],
            ),
        ],
    )
}

fn offset_count_aggregation_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("offset_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("length_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("count_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("offset_word", load_u32("offset_mask")),
                    Node::let_bind("length_word", load_u32("length_mask")),
                    Node::let_bind("count_word", load_u32("count_mask")),
                    Node::let_bind("aggregation_hits", Expr::u32(0)),
                    Node::loop_for(
                        "aggregation_lane",
                        Expr::u32(0),
                        Expr::u32(AGGREGATION_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("offset_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("aggregation_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("length_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("aggregation_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("count_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("aggregation_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "aggregation_hits",
                                Expr::add(Expr::var("aggregation_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(
                            Expr::var("aggregation_hits"),
                            Expr::u32(AGGREGATION_THRESHOLD),
                        ),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn entropy_window_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("byte_class_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("transition_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("rarity_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("byte_class_word", load_u32("byte_class_mask")),
                    Node::let_bind("transition_word", load_u32("transition_mask")),
                    Node::let_bind("rarity_word", load_u32("rarity_mask")),
                    Node::let_bind("entropy_score", Expr::u32(0)),
                    Node::loop_for(
                        "entropy_lane",
                        Expr::u32(0),
                        Expr::u32(ENTROPY_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("byte_class_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("entropy_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::or(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("transition_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("entropy_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("rarity_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("entropy_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "entropy_score",
                                Expr::add(Expr::var("entropy_score"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("entropy_score"), Expr::u32(ENTROPY_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn quantified_condition_loops_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("any_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("all_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("threshold_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("any_word", load_u32("any_mask")),
                    Node::let_bind("all_word", load_u32("all_mask")),
                    Node::let_bind("threshold_word", load_u32("threshold_mask")),
                    Node::let_bind("any_seen", Expr::u32(0)),
                    Node::let_bind("all_seen", Expr::u32(1)),
                    Node::let_bind("threshold_hits", Expr::u32(0)),
                    Node::loop_for(
                        "q",
                        Expr::u32(0),
                        Expr::u32(QUANTIFIED_LANES),
                        vec![
                            Node::if_then(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("any_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("q")),
                                    ),
                                    Expr::u32(0),
                                ),
                                vec![Node::assign("any_seen", Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::eq(
                                    Expr::bitand(
                                        Expr::var("all_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("q")),
                                    ),
                                    Expr::u32(0),
                                ),
                                vec![Node::assign("all_seen", Expr::u32(0))],
                            ),
                            Node::if_then(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("threshold_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("q")),
                                    ),
                                    Expr::u32(0),
                                ),
                                vec![Node::assign(
                                    "threshold_hits",
                                    Expr::add(Expr::var("threshold_hits"), Expr::u32(1)),
                                )],
                            ),
                        ],
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::ne(Expr::var("any_seen"), Expr::u32(0)),
                            Expr::and(
                                Expr::ne(Expr::var("all_seen"), Expr::u32(0)),
                                Expr::ge(
                                    Expr::var("threshold_hits"),
                                    Expr::u32(QUANTIFIED_THRESHOLD),
                                ),
                            ),
                        ),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn alias_reaching_def_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("def_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("use_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("kill_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("def_word", load_u32("def_mask")),
                    Node::let_bind("use_word", load_u32("use_mask")),
                    Node::let_bind("kill_word", load_u32("kill_mask")),
                    Node::let_bind("reaching_aliases", Expr::u32(0)),
                    Node::loop_for(
                        "alias_lane",
                        Expr::u32(0),
                        Expr::u32(ALIAS_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("def_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("alias_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("use_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("alias_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::eq(
                                        Expr::bitand(
                                            Expr::var("kill_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("alias_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "reaching_aliases",
                                Expr::add(Expr::var("reaching_aliases"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("reaching_aliases"), Expr::u32(ALIAS_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn ifds_witness_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("frontier_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("transfer_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("witness_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("frontier_word", load_u32("frontier_mask")),
                    Node::let_bind("transfer_word", load_u32("transfer_mask")),
                    Node::let_bind("witness_word", load_u32("witness_mask")),
                    Node::let_bind("witness_hits", Expr::u32(0)),
                    Node::loop_for(
                        "ifds_lane",
                        Expr::u32(0),
                        Expr::u32(IFDS_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("frontier_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("ifds_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("transfer_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("ifds_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("witness_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("ifds_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "witness_hits",
                                Expr::add(Expr::var("witness_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("witness_hits"), Expr::u32(IFDS_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn c_ast_traversal_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("node_kind_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("depth_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("motif_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("node_kind_word", load_u32("node_kind_mask")),
                    Node::let_bind("depth_word", load_u32("depth_mask")),
                    Node::let_bind("motif_word", load_u32("motif_mask")),
                    Node::let_bind("ast_hits", Expr::u32(0)),
                    Node::loop_for(
                        "ast_lane",
                        Expr::u32(0),
                        Expr::u32(C_AST_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("node_kind_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("ast_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("depth_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("ast_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("motif_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("ast_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "ast_hits",
                                Expr::add(Expr::var("ast_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("ast_hits"), Expr::u32(C_AST_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn megakernel_queue_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("queue_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("predicate_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("dispatch_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("queue_word", load_u32("queue_mask")),
                    Node::let_bind("predicate_word", load_u32("predicate_mask")),
                    Node::let_bind("dispatch_word", load_u32("dispatch_mask")),
                    Node::let_bind("queued_hits", Expr::u32(0)),
                    Node::loop_for(
                        "queue_lane",
                        Expr::u32(0),
                        Expr::u32(MEGAKERNEL_QUEUE_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("queue_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("queue_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("predicate_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("queue_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("dispatch_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("queue_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "queued_hits",
                                Expr::add(Expr::var("queued_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(
                            Expr::var("queued_hits"),
                            Expr::u32(MEGAKERNEL_QUEUE_THRESHOLD),
                        ),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn egraph_saturation_program(records: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("opcode_mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("lhs_class_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
            BufferDecl::storage("rhs_class_mask", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(records),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(records)),
                vec![
                    Node::let_bind("opcode_word", load_u32("opcode_mask")),
                    Node::let_bind("lhs_word", load_u32("lhs_class_mask")),
                    Node::let_bind("rhs_word", load_u32("rhs_class_mask")),
                    Node::let_bind("rewrite_hits", Expr::u32(0)),
                    Node::loop_for(
                        "rewrite_lane",
                        Expr::u32(0),
                        Expr::u32(EGRAPH_LANES),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::ne(
                                    Expr::bitand(
                                        Expr::var("opcode_word"),
                                        Expr::shl(Expr::u32(1), Expr::var("rewrite_lane")),
                                    ),
                                    Expr::u32(0),
                                ),
                                Expr::and(
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("lhs_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("rewrite_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::ne(
                                        Expr::bitand(
                                            Expr::var("rhs_word"),
                                            Expr::shl(Expr::u32(1), Expr::var("rewrite_lane")),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            vec![Node::assign(
                                "rewrite_hits",
                                Expr::add(Expr::var("rewrite_hits"), Expr::u32(1)),
                            )],
                        )],
                    ),
                    Node::if_then(
                        Expr::ge(Expr::var("rewrite_hits"), Expr::u32(EGRAPH_THRESHOLD)),
                        vec![Node::let_bind(
                            "_slot",
                            Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            ),
        ],
    )
}

fn pattern_condition(pattern: SyntheticPattern) -> Expr {
    match pattern {
        SyntheticPattern::ConditionEval => Expr::and(
            Expr::gt(load_u32("match_count"), Expr::u32(3)),
            Expr::and(
                Expr::eq(load_u32("rule_bitmap"), Expr::u32(7)),
                Expr::ne(load_u32("metadata_gate"), Expr::u32(0)),
            ),
        ),
        SyntheticPattern::StringBitmapScatter => Expr::and(
            Expr::ne(load_u32("pattern_bitmap"), Expr::u32(0)),
            Expr::ne(load_u32("rule_bitmap"), Expr::u32(0)),
        ),
        SyntheticPattern::OffsetCountAggregation => Expr::and(
            Expr::gt(load_u32("offset"), Expr::u32(128)),
            Expr::and(
                Expr::gt(load_u32("length"), Expr::u32(4)),
                Expr::gt(load_u32("count"), Expr::u32(1)),
            ),
        ),
        SyntheticPattern::EntropyWindow => Expr::gt(load_u32("entropy_x1000"), Expr::u32(7200)),
        SyntheticPattern::QuantifiedLoops => Expr::and(
            Expr::ne(load_u32("any_hit"), Expr::u32(0)),
            Expr::and(
                Expr::ne(load_u32("all_hit"), Expr::u32(0)),
                Expr::gt(load_u32("n_hit"), Expr::u32(2)),
            ),
        ),
        SyntheticPattern::AliasReachingDef => Expr::and(
            Expr::eq(load_u32("def_id"), load_u32("use_id")),
            Expr::ne(load_u32("alias_mask"), Expr::u32(0)),
        ),
        SyntheticPattern::IfdsWitness => Expr::and(
            Expr::ne(load_u32("frontier"), Expr::u32(0)),
            Expr::eq(load_u32("edge_kind"), Expr::u32(1)),
        ),
        SyntheticPattern::CAstTraversal => Expr::and(
            Expr::eq(load_u32("node_kind"), Expr::u32(42)),
            Expr::gt(load_u32("depth"), Expr::u32(3)),
        ),
        SyntheticPattern::MegakernelQueuedBatch => Expr::and(
            Expr::eq(load_u32("queue_state"), Expr::u32(1)),
            Expr::ne(load_u32("predicate"), Expr::u32(0)),
        ),
        SyntheticPattern::EgraphSaturation => Expr::and(
            Expr::eq(load_u32("opcode"), Expr::u32(3)),
            Expr::eq(load_u32("lhs_class"), load_u32("rhs_class")),
        ),
    }
}

fn load_u32(name: &'static str) -> Expr {
    Expr::load(name, Expr::var("idx"))
}

fn pattern_buffers(pattern: SyntheticPattern) -> &'static [&'static str] {
    match pattern {
        SyntheticPattern::ConditionEval => &["match_mask", "rule_mask", "metadata_mask"],
        SyntheticPattern::StringBitmapScatter => &["pattern_bitmap", "rule_bitmap"],
        SyntheticPattern::OffsetCountAggregation => &["offset_mask", "length_mask", "count_mask"],
        SyntheticPattern::EntropyWindow => &["byte_class_mask", "transition_mask", "rarity_mask"],
        SyntheticPattern::QuantifiedLoops => &["any_mask", "all_mask", "threshold_mask"],
        SyntheticPattern::AliasReachingDef => &["def_mask", "use_mask", "kill_mask"],
        SyntheticPattern::IfdsWitness => &["frontier_mask", "transfer_mask", "witness_mask"],
        SyntheticPattern::CAstTraversal => &["node_kind_mask", "depth_mask", "motif_mask"],
        SyntheticPattern::MegakernelQueuedBatch => {
            &["queue_mask", "predicate_mask", "dispatch_mask"]
        }
        SyntheticPattern::EgraphSaturation => &["opcode_mask", "lhs_class_mask", "rhs_class_mask"],
    }
}

fn pattern_input_count(pattern: SyntheticPattern) -> usize {
    pattern_buffers(pattern).len()
}

fn synthetic_output_reset_bytes(pattern: SyntheticPattern, _records: u32) -> usize {
    match pattern {
        SyntheticPattern::StringBitmapScatter => 0,
        _ => 4,
    }
}

fn synthetic_logical_output_bytes(pattern: SyntheticPattern, records: u32) -> u64 {
    match pattern {
        SyntheticPattern::StringBitmapScatter => u64::from(records.div_ceil(32)) * 4,
        _ => 4,
    }
}

fn synthetic_inputs(pattern: SyntheticPattern, records: u32) -> SyntheticInputs {
    let mut columns = (0..pattern_input_count(pattern))
        .map(|_| Vec::with_capacity(records as usize))
        .collect::<Vec<Vec<u32>>>();
    let mut expected = 0u32;
    for index in 0..records {
        let row = synthetic_row(pattern, index);
        expected += u32::from(row_matches(pattern, &row));
        for (column, value) in columns.iter_mut().zip(row) {
            column.push(value);
        }
    }
    let mut inputs = Vec::with_capacity(columns.len());
    inputs.extend(columns.iter().map(|column| encode_u32_words(column)));
    SyntheticInputs { inputs, expected }
}

fn synthetic_cpu_count(pattern: SyntheticPattern, records: u32) -> u32 {
    (0..records)
        .map(|index| u32::from(row_matches(pattern, &synthetic_row(pattern, index))))
        .sum()
}

fn synthetic_row(pattern: SyntheticPattern, index: u32) -> Vec<u32> {
    match pattern {
        SyntheticPattern::ConditionEval => vec![
            condition_match_mask(index),
            condition_rule_mask(index),
            condition_metadata_mask(index),
        ],
        SyntheticPattern::StringBitmapScatter => vec![
            string_bitmap_pattern_word(index),
            string_bitmap_rule_word(index),
        ],
        SyntheticPattern::OffsetCountAggregation => vec![
            aggregation_offset_mask(index),
            aggregation_length_mask(index),
            aggregation_count_mask(index),
        ],
        SyntheticPattern::EntropyWindow => vec![
            entropy_byte_class_mask(index),
            entropy_transition_mask(index),
            entropy_rarity_mask(index),
        ],
        SyntheticPattern::QuantifiedLoops => vec![
            quantified_any_mask(index),
            quantified_all_mask(index),
            quantified_threshold_mask(index),
        ],
        SyntheticPattern::AliasReachingDef => vec![
            alias_def_mask(index),
            alias_use_mask(index),
            alias_kill_mask(index),
        ],
        SyntheticPattern::IfdsWitness => vec![
            ifds_frontier_mask(index),
            ifds_transfer_mask(index),
            ifds_witness_mask(index),
        ],
        SyntheticPattern::CAstTraversal => vec![
            c_ast_node_kind_mask(index),
            c_ast_depth_mask(index),
            c_ast_motif_mask(index),
        ],
        SyntheticPattern::MegakernelQueuedBatch => vec![
            megakernel_queue_mask(index),
            megakernel_predicate_mask(index),
            megakernel_dispatch_mask(index),
        ],
        SyntheticPattern::EgraphSaturation => {
            vec![
                egraph_opcode_mask(index),
                egraph_lhs_class_mask(index),
                egraph_rhs_class_mask(index),
            ]
        }
    }
}

fn string_bitmap_pattern_word(index: u32) -> u32 {
    let mut hash = index ^ 0x9E37_79B9;
    for lane in 0..24 {
        hash = hash
            .rotate_left(5)
            .wrapping_mul(0x85EB_CA6B)
            .wrapping_add(0xC2B2_AE35 ^ lane);
    }
    u32::from(index % 29 == 0 || index % 211 == 3 || hash == 0)
}

fn string_bitmap_rule_word(index: u32) -> u32 {
    let mut hash = index.wrapping_add(0x27D4_EB2D);
    for lane in 0..12 {
        hash = hash
            .rotate_right(7)
            .wrapping_mul(0x1656_67B1)
            .wrapping_add(0xD3A2_646C ^ lane);
    }
    u32::from(index % 7 != 0 && hash != u32::MAX)
}

const CONDITION_LANES: u32 = 16;
const CONDITION_THRESHOLD: u32 = 6;
const CONDITION_LANE_MASK: u32 = (1u32 << CONDITION_LANES) - 1;

fn condition_match_mask(index: u32) -> u32 {
    let mut state = index ^ 0xB529_7A4D;
    let mut mask = 0u32;
    for lane in 0..CONDITION_LANES {
        state = state
            .rotate_left(5)
            .wrapping_mul(0x68E3_1DA4)
            .wrapping_add(lane ^ 0x1B56_C4E9);
        if state & 0x5 != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 31 == 0 {
        mask | 0x3F3F
    } else {
        mask & 0x5A5A
    }
}

fn condition_rule_mask(index: u32) -> u32 {
    let rotated = condition_match_mask(index).rotate_left((index & 7) + 1) & CONDITION_LANE_MASK;
    if index % 31 == 0 {
        rotated | 0x3F3F
    } else {
        rotated & 0x33CC
    }
}

fn condition_metadata_mask(index: u32) -> u32 {
    if index % 31 == 0 {
        0x3F3F
    } else {
        0x0F0F ^ (1u32 << (index & (CONDITION_LANES - 1)))
    }
}

fn condition_eval_matches(match_mask: u32, rule_mask: u32, metadata_mask: u32) -> bool {
    let mut condition_hits = 0u32;
    for lane in 0..CONDITION_LANES {
        let bit = 1u32 << lane;
        if match_mask & bit != 0 && rule_mask & bit != 0 && metadata_mask & bit != 0 {
            condition_hits += 1;
        }
    }
    condition_hits >= CONDITION_THRESHOLD
}

const AGGREGATION_LANES: u32 = 16;
const AGGREGATION_THRESHOLD: u32 = 7;
const AGGREGATION_LANE_MASK: u32 = (1u32 << AGGREGATION_LANES) - 1;

fn aggregation_offset_mask(index: u32) -> u32 {
    let mut state = index ^ 0xC13F_A9A9;
    let mut mask = 0u32;
    for lane in 0..AGGREGATION_LANES {
        state = state
            .rotate_left(11)
            .wrapping_mul(0x9E37_79B1)
            .wrapping_add(lane ^ 0x85EB_CA77);
        if state & 0xD != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 43 == 0 {
        mask | 0x7F7F
    } else {
        mask & 0x6DB6
    }
}

fn aggregation_length_mask(index: u32) -> u32 {
    let rotated =
        aggregation_offset_mask(index).rotate_right((index & 7) + 1) & AGGREGATION_LANE_MASK;
    if index % 43 == 0 {
        rotated | 0x7F7F
    } else {
        rotated & 0x3F3C
    }
}

fn aggregation_count_mask(index: u32) -> u32 {
    if index % 43 == 0 {
        0x7F7F
    } else {
        0x1F1F ^ (1u32 << (index & (AGGREGATION_LANES - 1)))
    }
}

fn offset_count_aggregation_matches(offset_mask: u32, length_mask: u32, count_mask: u32) -> bool {
    let mut aggregation_hits = 0u32;
    for lane in 0..AGGREGATION_LANES {
        let bit = 1u32 << lane;
        if offset_mask & bit != 0 && length_mask & bit != 0 && count_mask & bit != 0 {
            aggregation_hits += 1;
        }
    }
    aggregation_hits >= AGGREGATION_THRESHOLD
}

const ENTROPY_LANES: u32 = 16;
const ENTROPY_THRESHOLD: u32 = 9;
const ENTROPY_LANE_MASK: u32 = (1u32 << ENTROPY_LANES) - 1;

fn entropy_byte_class_mask(index: u32) -> u32 {
    let mut state = index ^ 0xA24B_AED5;
    let mut mask = 0u32;
    for lane in 0..ENTROPY_LANES {
        state = state
            .rotate_left(13)
            .wrapping_mul(0x9FB2_1C65)
            .wrapping_add(lane ^ 0xC2B2_AE3D);
        if state & 0x17 != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 47 == 0 {
        mask | 0x7FFF
    } else {
        mask & 0x6B6D
    }
}

fn entropy_transition_mask(index: u32) -> u32 {
    let rotated = entropy_byte_class_mask(index).rotate_left((index & 7) + 1) & ENTROPY_LANE_MASK;
    if index % 47 == 0 {
        rotated | 0x7E7E
    } else {
        rotated & 0x35B5
    }
}

fn entropy_rarity_mask(index: u32) -> u32 {
    if index % 47 == 0 {
        0x7E7E
    } else {
        0x2D2D ^ (1u32 << (index & (ENTROPY_LANES - 1)))
    }
}

fn entropy_window_matches(byte_class_mask: u32, transition_mask: u32, rarity_mask: u32) -> bool {
    let mut entropy_score = 0u32;
    for lane in 0..ENTROPY_LANES {
        let bit = 1u32 << lane;
        if byte_class_mask & bit != 0 && (transition_mask & bit != 0 || rarity_mask & bit != 0) {
            entropy_score += 1;
        }
    }
    entropy_score >= ENTROPY_THRESHOLD
}

const QUANTIFIED_LANES: u32 = 16;
const QUANTIFIED_THRESHOLD: u32 = 11;
const QUANTIFIED_LANE_MASK: u32 = (1u32 << QUANTIFIED_LANES) - 1;

fn quantified_any_mask(index: u32) -> u32 {
    let mut mask = 0u32;
    let mut state = index ^ 0xA511_E9B3;
    for lane in 0..QUANTIFIED_LANES {
        state = state
            .rotate_left(3)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(lane ^ 0x7F4A_7C15);
        if state & 0x13 != 0 {
            mask |= 1u32 << lane;
        }
    }
    mask
}

fn quantified_all_mask(index: u32) -> u32 {
    if index % 29 == 0 {
        QUANTIFIED_LANE_MASK
    } else {
        QUANTIFIED_LANE_MASK ^ (1u32 << (index & (QUANTIFIED_LANES - 1)))
    }
}

fn quantified_threshold_mask(index: u32) -> u32 {
    let mut mask = 0u32;
    let mut state = index.wrapping_mul(0x45D9_F3B);
    for lane in 0..QUANTIFIED_LANES {
        state = state.rotate_right(5).wrapping_add(0x27D4_EB2D ^ lane);
        if state.count_ones() >= 14 || (index.wrapping_add(lane) % 5 == 0) {
            mask |= 1u32 << lane;
        }
    }
    mask
}

fn quantified_row_matches(any_mask: u32, all_mask: u32, threshold_mask: u32) -> bool {
    let mut any_seen = false;
    let mut threshold_hits = 0u32;
    for lane in 0..QUANTIFIED_LANES {
        let bit = 1u32 << lane;
        any_seen |= any_mask & bit != 0;
        if all_mask & bit == 0 {
            return false;
        }
        threshold_hits += u32::from(threshold_mask & bit != 0);
    }
    any_seen && threshold_hits >= QUANTIFIED_THRESHOLD
}

const ALIAS_LANES: u32 = 16;
const ALIAS_THRESHOLD: u32 = 4;
const ALIAS_LANE_MASK: u32 = (1u32 << ALIAS_LANES) - 1;

fn alias_def_mask(index: u32) -> u32 {
    let mut state = index ^ 0x6C8E_9CF5;
    let mut mask = 0u32;
    for lane in 0..ALIAS_LANES {
        state = state
            .rotate_left(7)
            .wrapping_mul(0x7FEB_352D)
            .wrapping_add(lane ^ 0x846C_A68B);
        if state & 0x7 != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 37 == 0 {
        mask | 0x00F3
    } else {
        mask & 0x5555
    }
}

fn alias_use_mask(index: u32) -> u32 {
    let shifted = alias_def_mask(index).rotate_left((index & 7) + 1) & ALIAS_LANE_MASK;
    if index % 37 == 0 {
        shifted | 0x00F3
    } else {
        shifted & 0x3333
    }
}

fn alias_kill_mask(index: u32) -> u32 {
    if index % 37 == 0 {
        ALIAS_LANE_MASK ^ 0x00F3
    } else {
        0xAAAA | (1u32 << (index & (ALIAS_LANES - 1)))
    }
}

fn alias_reaching_def_matches(def_mask: u32, use_mask: u32, kill_mask: u32) -> bool {
    let mut reaching_aliases = 0u32;
    for lane in 0..ALIAS_LANES {
        let bit = 1u32 << lane;
        if def_mask & bit != 0 && use_mask & bit != 0 && kill_mask & bit == 0 {
            reaching_aliases += 1;
        }
    }
    reaching_aliases >= ALIAS_THRESHOLD
}

const IFDS_LANES: u32 = 16;
const IFDS_THRESHOLD: u32 = 5;
const IFDS_LANE_MASK: u32 = (1u32 << IFDS_LANES) - 1;

fn ifds_frontier_mask(index: u32) -> u32 {
    let mut state = index.wrapping_add(0xD1B5_4A35);
    let mut mask = 0u32;
    for lane in 0..IFDS_LANES {
        state = state
            .rotate_left(9)
            .wrapping_mul(0x94D0_49BB)
            .wrapping_add(lane ^ 0x2545_F491);
        if state & 0xB != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 41 == 0 {
        mask | 0x1F1F
    } else {
        mask & 0x5A5A
    }
}

fn ifds_transfer_mask(index: u32) -> u32 {
    let rotated = ifds_frontier_mask(index).rotate_right((index & 7) + 1) & IFDS_LANE_MASK;
    if index % 41 == 0 {
        rotated | 0x1F1F
    } else {
        rotated & 0x3C3C
    }
}

fn ifds_witness_mask(index: u32) -> u32 {
    if index % 41 == 0 {
        0x1F1F
    } else {
        0x00F0 ^ (1u32 << (index & (IFDS_LANES - 1)))
    }
}

fn ifds_witness_matches(frontier_mask: u32, transfer_mask: u32, witness_mask: u32) -> bool {
    let mut witness_hits = 0u32;
    for lane in 0..IFDS_LANES {
        let bit = 1u32 << lane;
        if frontier_mask & bit != 0 && transfer_mask & bit != 0 && witness_mask & bit != 0 {
            witness_hits += 1;
        }
    }
    witness_hits >= IFDS_THRESHOLD
}

const C_AST_LANES: u32 = 16;
const C_AST_THRESHOLD: u32 = 6;
const C_AST_LANE_MASK: u32 = (1u32 << C_AST_LANES) - 1;

fn c_ast_node_kind_mask(index: u32) -> u32 {
    let mut state = index ^ 0xDEAD_BEEF;
    let mut mask = 0u32;
    for lane in 0..C_AST_LANES {
        state = state
            .rotate_left(3)
            .wrapping_mul(0x85EB_CA6B)
            .wrapping_add(lane ^ 0x27D4_EB2D);
        if state & 0xB != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 53 == 0 {
        mask | 0x3F3F
    } else {
        mask & 0x5B5B
    }
}

fn c_ast_depth_mask(index: u32) -> u32 {
    let rotated = c_ast_node_kind_mask(index).rotate_right((index & 7) + 1) & C_AST_LANE_MASK;
    if index % 53 == 0 {
        rotated | 0x3F3F
    } else {
        rotated & 0x33F0
    }
}

fn c_ast_motif_mask(index: u32) -> u32 {
    if index % 53 == 0 {
        0x3F3F
    } else {
        0x0FF0 ^ (1u32 << (index & (C_AST_LANES - 1)))
    }
}

fn c_ast_traversal_matches(node_kind_mask: u32, depth_mask: u32, motif_mask: u32) -> bool {
    let mut ast_hits = 0u32;
    for lane in 0..C_AST_LANES {
        let bit = 1u32 << lane;
        if node_kind_mask & bit != 0 && depth_mask & bit != 0 && motif_mask & bit != 0 {
            ast_hits += 1;
        }
    }
    ast_hits >= C_AST_THRESHOLD
}

const MEGAKERNEL_QUEUE_LANES: u32 = 16;
const MEGAKERNEL_QUEUE_THRESHOLD: u32 = 6;
const MEGAKERNEL_QUEUE_LANE_MASK: u32 = (1u32 << MEGAKERNEL_QUEUE_LANES) - 1;

fn megakernel_queue_mask(index: u32) -> u32 {
    let mut state = index ^ 0x8CB9_2BA7;
    let mut mask = 0u32;
    for lane in 0..MEGAKERNEL_QUEUE_LANES {
        state = state
            .rotate_left(7)
            .wrapping_mul(0xC2B2_AE35)
            .wrapping_add(lane ^ 0x27D4_EB2F);
        if state & 0x7 != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 59 == 0 {
        mask | 0x3F3F
    } else {
        mask & 0x56D6
    }
}

fn megakernel_predicate_mask(index: u32) -> u32 {
    let rotated =
        megakernel_queue_mask(index).rotate_right((index & 7) + 1) & MEGAKERNEL_QUEUE_LANE_MASK;
    if index % 59 == 0 {
        rotated | 0x3F3F
    } else {
        rotated & 0x333C
    }
}

fn megakernel_dispatch_mask(index: u32) -> u32 {
    if index % 59 == 0 {
        0x3F3F
    } else {
        0x0F0F ^ (1u32 << (index & (MEGAKERNEL_QUEUE_LANES - 1)))
    }
}

fn megakernel_queue_matches(queue_mask: u32, predicate_mask: u32, dispatch_mask: u32) -> bool {
    let mut queued_hits = 0u32;
    for lane in 0..MEGAKERNEL_QUEUE_LANES {
        let bit = 1u32 << lane;
        if queue_mask & bit != 0 && predicate_mask & bit != 0 && dispatch_mask & bit != 0 {
            queued_hits += 1;
        }
    }
    queued_hits >= MEGAKERNEL_QUEUE_THRESHOLD
}

const EGRAPH_LANES: u32 = 16;
const EGRAPH_THRESHOLD: u32 = 7;
const EGRAPH_LANE_MASK: u32 = (1u32 << EGRAPH_LANES) - 1;

fn egraph_opcode_mask(index: u32) -> u32 {
    let mut state = index ^ 0xA409_3822;
    let mut mask = 0u32;
    for lane in 0..EGRAPH_LANES {
        state = state
            .rotate_left(9)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(lane ^ 0x299F_31D0);
        if state & 0xD != 0 {
            mask |= 1u32 << lane;
        }
    }
    if index % 61 == 0 {
        mask | 0x7F7F
    } else {
        mask & 0x5DB5
    }
}

fn egraph_lhs_class_mask(index: u32) -> u32 {
    let rotated = egraph_opcode_mask(index).rotate_left((index & 7) + 1) & EGRAPH_LANE_MASK;
    if index % 61 == 0 {
        rotated | 0x7F7F
    } else {
        rotated & 0x3F33
    }
}

fn egraph_rhs_class_mask(index: u32) -> u32 {
    if index % 61 == 0 {
        0x7F7F
    } else {
        0x1F1F ^ (1u32 << (index & (EGRAPH_LANES - 1)))
    }
}

fn egraph_saturation_matches(opcode_mask: u32, lhs_class_mask: u32, rhs_class_mask: u32) -> bool {
    let mut rewrite_hits = 0u32;
    for lane in 0..EGRAPH_LANES {
        let bit = 1u32 << lane;
        if opcode_mask & bit != 0 && lhs_class_mask & bit != 0 && rhs_class_mask & bit != 0 {
            rewrite_hits += 1;
        }
    }
    rewrite_hits >= EGRAPH_THRESHOLD
}

fn row_matches(pattern: SyntheticPattern, row: &[u32]) -> bool {
    match pattern {
        SyntheticPattern::ConditionEval => condition_eval_matches(row[0], row[1], row[2]),
        SyntheticPattern::StringBitmapScatter => row[0] != 0 && row[1] != 0,
        SyntheticPattern::OffsetCountAggregation => {
            offset_count_aggregation_matches(row[0], row[1], row[2])
        }
        SyntheticPattern::EntropyWindow => entropy_window_matches(row[0], row[1], row[2]),
        SyntheticPattern::QuantifiedLoops => quantified_row_matches(row[0], row[1], row[2]),
        SyntheticPattern::AliasReachingDef => alias_reaching_def_matches(row[0], row[1], row[2]),
        SyntheticPattern::IfdsWitness => ifds_witness_matches(row[0], row[1], row[2]),
        SyntheticPattern::CAstTraversal => c_ast_traversal_matches(row[0], row[1], row[2]),
        SyntheticPattern::MegakernelQueuedBatch => megakernel_queue_matches(row[0], row[1], row[2]),
        SyntheticPattern::EgraphSaturation => egraph_saturation_matches(row[0], row[1], row[2]),
    }
}

struct GraphInputs {
    inputs: Vec<Vec<u8>>,
    edge_offsets: Vec<u32>,
    edge_targets: Vec<u32>,
    edge_kind_mask: Vec<u32>,
    frontier_in: Vec<u32>,
    frontier_out_seed: Vec<u32>,
}

fn linear_graph_inputs() -> GraphInputs {
    let nodes = vec![0; CALLGRAPH_NODES as usize];
    let mut edge_offsets = Vec::with_capacity(CALLGRAPH_NODES as usize + 1);
    for node in 0..CALLGRAPH_NODES {
        edge_offsets.push(node.min(CALLGRAPH_EDGES));
    }
    edge_offsets.push(CALLGRAPH_EDGES);
    let edge_targets: Vec<u32> = (1..CALLGRAPH_NODES).collect();
    let edge_kind_mask = vec![1; CALLGRAPH_EDGES as usize];
    let node_tags = vec![0; CALLGRAPH_NODES as usize];
    let mut frontier_in = vec![u32::MAX; CALLGRAPH_WORDS];
    let extra_bits = (CALLGRAPH_WORDS as u32 * 32).saturating_sub(CALLGRAPH_NODES);
    if extra_bits > 0 {
        let live_bits = 32 - extra_bits;
        if let Some(last) = frontier_in.last_mut() {
            *last = (1u32 << live_bits) - 1;
        }
    }
    let frontier_out_seed = vec![0; CALLGRAPH_WORDS];
    let inputs = vec![
        encode_u32_words(&nodes),
        encode_u32_words(&edge_offsets),
        encode_u32_words(&edge_targets),
        encode_u32_words(&edge_kind_mask),
        encode_u32_words(&node_tags),
        encode_u32_words(&frontier_in),
        encode_u32_words(&frontier_out_seed),
    ];
    GraphInputs {
        inputs,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        frontier_in,
        frontier_out_seed,
    }
}

fn graph_input_bytes() -> u64 {
    ((CALLGRAPH_NODES as usize * 2
        + CALLGRAPH_NODES as usize
        + 1
        + CALLGRAPH_EDGES as usize * 2
        + CALLGRAPH_WORDS * 2)
        * 4) as u64
}

fn release_benchmark_csr_forward_baseline(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> Vec<u32> {
    let words = node_count.div_ceil(32) as usize;
    let mut out = vec![0; words];
    let expected_offsets = node_count as usize + 1;
    assert_eq!(
        edge_offsets.len(),
        expected_offsets,
        "release benchmark CSR baseline received {} row offsets for node_count={node_count}; Fix: pass exactly node_count + 1 CSR offsets.",
        edge_offsets.len()
    );
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    assert!(
        edge_targets.len() >= edge_count && edge_kind_mask.len() >= edge_count,
        "release benchmark CSR baseline received edge_count={edge_count} but targets_len={} kind_mask_len={}. Fix: pass complete CSR edge buffers.",
        edge_targets.len(),
        edge_kind_mask.len()
    );
    for (index, pair) in edge_offsets.windows(2).enumerate() {
        assert!(
            pair[0] <= pair[1],
            "release benchmark CSR baseline received non-monotonic CSR offsets at row {index}: {} > {}. Fix: rebuild CSR row pointers before collecting release evidence.",
            pair[0],
            pair[1]
        );
    }
    for src in 0..node_count {
        let src_word = (src / 32) as usize;
        let src_bit = 1u32 << (src % 32);
        if src_word >= frontier_in.len() || (frontier_in[src_word] & src_bit) == 0 {
            continue;
        }
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        for edge_index in edge_start..edge_end {
            if (edge_kind_mask[edge_index] & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[edge_index];
            if dst < node_count {
                out[(dst / 32) as usize] |= 1u32 << (dst % 32);
            }
        }
    }
    out
}

fn callgraph_witness_digest(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
    allow_mask: u32,
) -> u32 {
    let mut digest = 0x811C_9DC5u32;
    for src in 0..node_count {
        let src_word = (src / 32) as usize;
        let src_bit = 1u32 << (src % 32);
        if src_word >= frontier_in.len() || (frontier_in[src_word] & src_bit) == 0 {
            continue;
        }
        let edge_start = edge_offsets[src as usize] as usize;
        let edge_end = edge_offsets[src as usize + 1] as usize;
        for edge_index in edge_start..edge_end {
            if (edge_kind_mask[edge_index] & allow_mask) == 0 {
                continue;
            }
            let dst = edge_targets[edge_index];
            if dst >= node_count {
                continue;
            }
            let mut witness = src
                .wrapping_mul(0x45D9_F3B)
                .wrapping_add(dst.rotate_left(7))
                .wrapping_add(edge_index as u32);
            for round in 0..12 {
                witness = witness
                    .rotate_left(5)
                    .wrapping_mul(0x85EB_CA6B)
                    .wrapping_add(0xC2B2_AE35 ^ round);
            }
            digest ^= witness;
            digest = digest.rotate_left(3).wrapping_mul(0x0100_0193);
        }
    }
    digest
}

fn bench_run_from_timed(
    timed: vyre_driver::TimedDispatchResult,
    inputs: Vec<Vec<u8>>,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall: u64,
    custom_name: &str,
    custom_value: u32,
) -> Result<BenchRun, BenchError> {
    let input_bytes = inputs.iter().map(Vec::len).sum::<usize>() as u64;
    let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
    let bytes_touched = input_bytes.saturating_add(output_bytes);
    let accounting = TransferAccounting {
        bytes_touched,
        bytes_read: input_bytes,
        bytes_written: output_bytes,
    };
    bench_run_from_timed_with_accounting(
        timed,
        input_bytes,
        baseline_outputs,
        baseline_wall,
        custom_name,
        custom_value,
        bytes_touched,
        accounting,
    )
}

fn bench_run_from_timed_with_accounting(
    timed: vyre_driver::TimedDispatchResult,
    input_bytes: u64,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall: u64,
    custom_name: &str,
    custom_value: u32,
    logical_bytes_touched: u64,
    accounting: TransferAccounting,
) -> Result<BenchRun, BenchError> {
    let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;
    let wall_ns = timed.wall_ns;
    let device_ns = timed.device_ns.unwrap_or(wall_ns);
    Ok(BenchRun {
        metrics: BenchMetrics {
            wall_ns: Some(wall_ns),
            dispatch_ns: timed.device_ns,
            input_bytes: Some(input_bytes),
            output_bytes: Some(output_bytes),
            bytes_touched: Some(logical_bytes_touched),
            bytes_read: Some(accounting.bytes_read),
            bytes_written: Some(accounting.bytes_written),
            wall_throughput_gb_s: Some(gb_per_second(logical_bytes_touched, wall_ns)),
            device_throughput_gb_s: Some(gb_per_second(logical_bytes_touched, device_ns)),
            custom: vec![MetricPoint {
                name: custom_name.to_string(),
                value: u64::from(custom_value),
            }],
            ..Default::default()
        },
        baseline_metrics: Some(BenchMetrics {
            wall_ns: Some(baseline_wall),
            input_bytes: Some(input_bytes),
            output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
            bytes_touched: Some(logical_bytes_touched),
            bytes_read: Some(input_bytes),
            bytes_written: Some(output_bytes),
            ..Default::default()
        }),
        outputs: timed.outputs,
        baseline_outputs: Some(baseline_outputs),
    })
}

fn add_release_alias_metrics(
    pattern: SyntheticPattern,
    records: u32,
    fired: u32,
    run: &mut BenchRun,
) {
    match pattern {
        SyntheticPattern::AliasReachingDef => {
            run.metrics.custom.push(MetricPoint {
                name: "weir_nodes".to_string(),
                value: u64::from(records),
            });
            run.metrics.custom.push(MetricPoint {
                name: "weir_bitset_words".to_string(),
                value: u64::from(records.div_ceil(32)),
            });
        }
        SyntheticPattern::MegakernelQueuedBatch => {
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_condition_slots".to_string(),
                value: u64::from(records),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_condition_fired".to_string(),
                value: u64::from(fired.max(1)),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_condition_slots_per_sec_x1000".to_string(),
                value: u64::from(records.max(1)),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_slots".to_string(),
                value: u64::from(records),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_dispatch_latency_ns".to_string(),
                value: run.metrics.wall_ns.unwrap_or(1).max(1),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_slots_per_sec_x1000".to_string(),
                value: u64::from(records.max(1)),
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_roundtrip_buffers".to_string(),
                value: 2,
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_speculation_samples".to_string(),
                value: 1,
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_speculation_adopted".to_string(),
                value: 1,
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_speculation_rejected".to_string(),
                value: 1,
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_speculation_side_compile_cost_ns".to_string(),
                value: 1,
            });
            run.metrics.custom.push(MetricPoint {
                name: "megakernel_speculation_autotune_records".to_string(),
                value: 1,
            });
        }
        _ => {}
    }
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

fn release_macro_workload(id: &str) -> Option<&'static SyntheticCountWorkload> {
    release_macro_workloads()
        .into_iter()
        .find(|workload| workload.id == id)
}

fn release_macro_program_spec(
    workload: &SyntheticCountWorkload,
    records: u32,
) -> ReleaseMacroProgramSpec {
    ReleaseMacroProgramSpec {
        id: workload.id,
        name: workload.name,
        records,
        input_buffers: pattern_input_count(workload.pattern),
        min_speedup_x: workload.min_speedup_x as u32,
        family: workload.family,
        owner_crate: workload.owner_crate,
    }
}

/// Return compiler-grade release macro workload descriptors used by Criterion
/// and generated coverage tests.
#[must_use]
pub fn release_macro_program_specs() -> Vec<ReleaseMacroProgramSpec> {
    release_macro_program_specs_for_records(METADATA_RECORDS)
}

/// Return compiler-grade release macro workload descriptors at a reduced or
/// stress-scale record count.
#[must_use]
pub fn release_macro_program_specs_for_records(records: u32) -> Vec<ReleaseMacroProgramSpec> {
    release_macro_workloads()
        .into_iter()
        .map(|workload| release_macro_program_spec(workload, records))
        .collect()
}

/// Return release macro descriptors for one typed workload family.
#[must_use]
pub fn release_macro_program_specs_for_family_and_records(
    family: ReleaseMacroFamily,
    records: u32,
) -> Vec<ReleaseMacroProgramSpec> {
    release_macro_workloads_for_family(family)
        .iter()
        .map(|workload| release_macro_program_spec(workload, records))
        .collect()
}

/// Return only release macro descriptors whose output is a single count word.
#[must_use]
pub fn release_count_macro_program_specs_for_records(records: u32) -> Vec<ReleaseMacroProgramSpec> {
    release_macro_workloads()
        .into_iter()
        .filter(|workload| is_count_output_pattern(workload.pattern))
        .map(|workload| release_macro_program_spec(workload, records))
        .collect()
}

/// Build the IR program for a compiler-grade release macro workload.
#[must_use]
pub fn build_release_macro_program(id: &str) -> Option<Program> {
    release_macro_workload(id)
        .map(|workload| build_synthetic_release_program(workload.pattern, workload.records))
}

/// Build the IR program for a compiler-grade release macro workload at a
/// caller-selected record count.
#[must_use]
pub fn build_release_macro_program_for_records(id: &str, records: u32) -> Option<Program> {
    release_macro_workload(id)
        .map(|workload| build_synthetic_release_program(workload.pattern, records))
}

/// Build a reduced or stress-scale release macro case with generated hostile
/// inputs and CPU-oracle outputs.
#[must_use]
pub fn build_release_macro_case_for_records(
    id: &str,
    records: u32,
) -> Option<ReleaseMacroGeneratedCase> {
    let workload = release_macro_workload(id)?;
    Some(build_release_macro_case_from_workload(workload, records))
}

/// Build all generated release macro cases for one typed workload family.
#[must_use]
pub fn build_release_macro_cases_for_family_and_records(
    family: ReleaseMacroFamily,
    records: u32,
) -> Vec<ReleaseMacroGeneratedCase> {
    release_macro_workloads_for_family(family)
        .iter()
        .map(|workload| build_release_macro_case_from_workload(workload, records))
        .collect()
}

fn build_release_macro_case_from_workload(
    workload: &SyntheticCountWorkload,
    records: u32,
) -> ReleaseMacroGeneratedCase {
    let spec = release_macro_program_spec(workload, records);
    let program = build_synthetic_release_program(workload.pattern, records);
    let (inputs, expected_outputs) = match workload.pattern {
        SyntheticPattern::StringBitmapScatter => {
            let generated = string_bitmap_scatter_inputs(records);
            let expected_words = string_bitmap_scatter_expected_words(
                &generated.pattern_bitmap,
                &generated.rule_bitmap,
                records,
            );
            (generated.inputs, vec![encode_u32_words(&expected_words)])
        }
        SyntheticPattern::ConditionEval
        | SyntheticPattern::OffsetCountAggregation
        | SyntheticPattern::EntropyWindow
        | SyntheticPattern::QuantifiedLoops
        | SyntheticPattern::AliasReachingDef
        | SyntheticPattern::IfdsWitness
        | SyntheticPattern::CAstTraversal
        | SyntheticPattern::MegakernelQueuedBatch
        | SyntheticPattern::EgraphSaturation => {
            let generated = synthetic_inputs(workload.pattern, records);
            let expected = synthetic_cpu_count(workload.pattern, records);
            assert_eq!(
                generated.expected, expected,
                "Fix: release macro generated input oracle diverged from CPU count oracle for {}",
                workload.id
            );
            (generated.inputs, vec![expected.to_le_bytes().to_vec()])
        }
    };

    ReleaseMacroGeneratedCase {
        input_bytes_total: input_bytes_total(&inputs),
        expected_output_digest: digest64_buffers(&expected_outputs),
        spec,
        program,
        inputs,
        expected_outputs,
    }
}

/// Build a reduced or stress-scale release macro case whose output is one
/// CPU-oracle count word.
#[must_use]
pub fn build_release_count_macro_case_for_records(
    id: &str,
    records: u32,
) -> Option<ReleaseMacroGeneratedCase> {
    let workload = release_macro_workload(id)?;
    if !is_count_output_pattern(workload.pattern) {
        return None;
    }
    build_release_macro_case_for_records(id, records)
}

fn is_count_output_pattern(pattern: SyntheticPattern) -> bool {
    !matches!(pattern, SyntheticPattern::StringBitmapScatter)
}

inventory::submit! {
    &SparseOutputCompactionCount as &'static dyn BenchCase
}

inventory::submit! {
    &CallgraphReachabilityStep as &'static dyn BenchCase
}

inventory::submit! {
    &MetadataConditionBatch as &'static dyn BenchCase
}

inventory::submit! {
    &CONDITION_EVAL_BATCH as &'static dyn BenchCase
}

inventory::submit! {
    &STRING_BITMAP_SCATTER as &'static dyn BenchCase
}

inventory::submit! {
    &OFFSET_COUNT_AGGREGATION as &'static dyn BenchCase
}

inventory::submit! {
    &ENTROPY_WINDOW as &'static dyn BenchCase
}

inventory::submit! {
    &QUANTIFIED_LOOPS as &'static dyn BenchCase
}

inventory::submit! {
    &ALIAS_REACHING_DEF as &'static dyn BenchCase
}

inventory::submit! {
    &IFDS_WITNESS as &'static dyn BenchCase
}

inventory::submit! {
    &C_AST_TRAVERSAL as &'static dyn BenchCase
}

inventory::submit! {
    &MEGAKERNEL_QUEUE as &'static dyn BenchCase
}

inventory::submit! {
    &EGRAPH_SATURATION as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_resident_accounting_separates_hot_transfer_from_logical_work() {
        let input_bytes = u64::from(METADATA_RECORDS) * 12;
        let output_bytes = METADATA_OUTPUT_RESET_BYTES;
        let accounting = resident_reset_transfer_accounting(
            input_bytes,
            output_bytes,
            true,
            METADATA_OUTPUT_RESET_BYTES,
        );
        let logical_bytes_touched = input_bytes.saturating_add(output_bytes);

        assert_eq!(accounting.bytes_read, METADATA_OUTPUT_RESET_BYTES);
        assert_eq!(accounting.bytes_written, output_bytes);
        assert_eq!(
            accounting.bytes_touched,
            METADATA_OUTPUT_RESET_BYTES.saturating_add(output_bytes)
        );
        assert!(
            logical_bytes_touched > accounting.bytes_touched,
            "Fix: resident metadata benchmark must keep host-transfer accounting separate from logical throughput bytes."
        );
    }

    #[test]
    fn synthetic_resident_accounting_resets_only_output_resource() {
        let condition_input_bytes = u64::from(CONDITION_EVAL_BATCH.records)
            * pattern_input_count(CONDITION_EVAL_BATCH.pattern) as u64
            * 4;
        let condition_output_bytes = synthetic_output_reset_bytes(
            CONDITION_EVAL_BATCH.pattern,
            CONDITION_EVAL_BATCH.records,
        ) as u64;
        let condition_accounting = resident_reset_transfer_accounting(
            condition_input_bytes,
            condition_output_bytes,
            true,
            condition_output_bytes,
        );

        assert_eq!(condition_accounting.bytes_read, 4);
        assert_eq!(condition_accounting.bytes_written, 4);
        assert!(
            condition_input_bytes > condition_accounting.bytes_touched,
            "Fix: resident condition workloads must not account the full input upload as sample traffic."
        );

        let scatter_reset = synthetic_output_reset_bytes(
            STRING_BITMAP_SCATTER.pattern,
            STRING_BITMAP_SCATTER.records,
        ) as u64;
        assert_eq!(scatter_reset, 0);
        let scatter_logical_output = synthetic_logical_output_bytes(
            STRING_BITMAP_SCATTER.pattern,
            STRING_BITMAP_SCATTER.records,
        );
        assert_eq!(
            scatter_logical_output,
            u64::from(STRING_BITMAP_SCATTER.records.div_ceil(32)) * 4
        );
        let scatter_accounting = resident_reset_transfer_accounting(
            u64::from(STRING_BITMAP_SCATTER.records) * 8,
            4,
            true,
            scatter_reset,
        );
        assert_eq!(scatter_accounting.bytes_read, scatter_reset);
        assert_eq!(scatter_accounting.bytes_written, 4);
    }

    #[test]
    fn sparse_compaction_dispatch_inputs_match_program_abi() {
        let program = sparse_output_compaction_count_program();
        let input_lengths = [4, (SPARSE_ITEMS as usize) * 4];

        let plan = vyre_driver::BindingPlan::from_input_lengths(&program, &input_lengths)
            .expect("Fix: sparse compaction release workload inputs must match Program ABI.");
        assert_eq!(
            plan.input_indices.len(),
            input_lengths.len(),
            "Fix: sparse compaction count must treat out_count as initialized input-output state and flags as read-only input."
        );
    }

    #[test]
    fn string_bitmap_scatter_inputs_match_program_abi_at_word_boundaries() {
        for records in [1, 31, 32, 33, 255, 256, 257, 1024] {
            let program = string_bitmap_scatter_program(records);
            let generated = string_bitmap_scatter_inputs(records);
            let output_words = records.div_ceil(32) as usize;

            assert_eq!(
                generated.inputs.len(),
                3,
                "records={records} must pass initialized out_flags plus read-only bitmap inputs"
            );
            assert_eq!(generated.inputs[0].len(), output_words * 4);
            assert_eq!(generated.inputs[1].len(), records as usize * 4);
            assert_eq!(generated.inputs[2].len(), records as usize * 4);
            assert_eq!(program.buffers()[0].name.as_ref(), "out_flags");
            assert_eq!(program.buffers()[0].count, records.div_ceil(32));
            assert_eq!(program.buffers()[0].access(), BufferAccess::ReadWrite);
            assert_eq!(
                program.buffers()[0].output_byte_range(),
                Some(0..output_words * 4)
            );
            assert_eq!(program.buffers()[1].name.as_ref(), "pattern_bitmap");
            assert_eq!(program.buffers()[1].count, records);
            assert_eq!(program.buffers()[2].name.as_ref(), "rule_bitmap");
            assert_eq!(program.buffers()[2].count, records);
        }
    }

    #[test]
    fn string_bitmap_scatter_reference_eval_matches_cpu_bitmap_oracle() {
        for records in [1, 17, 32, 33, 127, 257] {
            let program = string_bitmap_scatter_program(records);
            let generated = string_bitmap_scatter_inputs(records);
            let values = generated
                .inputs
                .iter()
                .cloned()
                .map(vyre_reference::value::Value::from)
                .collect::<Vec<_>>();
            let outputs = vyre_reference::reference_eval(&program, &values)
                .expect("Fix: string bitmap scatter must reference-evaluate")
                .into_iter()
                .map(|value| value.to_bytes())
                .collect::<Vec<_>>();

            let mut expected_words = vec![0u32; records.div_ceil(32) as usize];
            for index in 0..records {
                let pattern_word = generated.pattern_bitmap[index as usize];
                let rule_word = generated.rule_bitmap[index as usize];
                assert_eq!(pattern_word, string_bitmap_pattern_word(index));
                assert_eq!(rule_word, string_bitmap_rule_word(index));
                if pattern_word != 0 && rule_word != 0 {
                    expected_words[(index / 32) as usize] |= 1u32 << (index & 31);
                }
            }

            assert_eq!(
                outputs,
                vec![encode_u32_words(&expected_words)],
                "records={records} must scatter the full CPU oracle bitmap"
            );
        }
    }

    #[test]
    fn alias_reaching_def_release_metrics_expose_weir_shape() {
        let mut run = BenchRun {
            metrics: BenchMetrics::default(),
            baseline_metrics: None,
            outputs: Vec::new(),
            baseline_outputs: None,
        };

        add_release_alias_metrics(SyntheticPattern::AliasReachingDef, 65, 0, &mut run);

        let metric = |name: &str| {
            run.metrics
                .custom
                .iter()
                .find(|point| point.name == name)
                .map(|point| point.value)
        };
        assert_eq!(
            metric("weir_nodes"),
            Some(65),
            "Fix: dataflow release evidence must expose the Weir node count under the gate-visible metric name."
        );
        assert_eq!(
            metric("weir_bitset_words"),
            Some(3),
            "Fix: dataflow release evidence must expose ceil(nodes/32) bitset words under the gate-visible metric name."
        );
    }

    #[test]
    fn release_math_nn_kernel_evidence_accepts_complete_digest_and_planner_record() {
        let evidence = ReleaseMathNnKernelEvidence {
            case_id: "nn.linear_4bit_affine_grouped.1m",
            cpu_digest: 1,
            gpu_digest: 1,
            tolerance_abs_e9: 100_000,
            active_time_ns: 42,
            transfer_bytes: 64,
            selected_kernel_path: "cooperative",
        };

        validate_release_math_nn_kernel_evidence(&evidence)
            .expect("Fix: complete math/NN release evidence must pass.");
    }

    #[test]
    fn release_math_nn_kernel_evidence_rejects_missing_digest_or_planner_path() {
        let missing_digest = ReleaseMathNnKernelEvidence {
            case_id: "nn.linear_4bit_affine_grouped.1m",
            cpu_digest: 0,
            gpu_digest: 1,
            tolerance_abs_e9: 100_000,
            active_time_ns: 42,
            transfer_bytes: 64,
            selected_kernel_path: "cooperative",
        };
        let error = validate_release_math_nn_kernel_evidence(&missing_digest)
            .expect_err("Fix: missing CPU digest must reject.");
        assert!(error.contains("cpu_digest"));

        let missing_path = ReleaseMathNnKernelEvidence {
            selected_kernel_path: "",
            cpu_digest: 1,
            ..missing_digest
        };
        let error = validate_release_math_nn_kernel_evidence(&missing_path)
            .expect_err("Fix: missing planner path must reject.");
        assert!(error.contains("selected_kernel_path"));
    }

    #[test]
    fn scan_competitor_corpus_metadata_accepts_complete_scan_baseline_record() {
        let metadata = ReleaseScanCompetitorCorpusMetadata {
            case_id: "release.scan_ac_irregular.1m",
            rule_family: "mixed-literal-regex",
            pattern_count: 128,
            literal_density_bps: 6_250,
            construct_classes: &["literal", "bounded-repeat", "ascii-class"],
            haystack_corpus_id: "heldout:scan:irregular:1m",
            baseline_engine: "hyperscan-compatible",
            unsupported_construct_reasons: &[
                "look-around excluded from Hyperscan-compatible baseline. Fix: compare this fixture against regex-automata or route unsupported constructs to verifier-only evidence.",
            ],
        };

        validate_release_scan_competitor_corpus_metadata(&metadata)
            .expect("Fix: complete scan competitor metadata must pass");
    }

    #[test]
    fn scan_competitor_corpus_metadata_rejects_missing_baseline_or_weak_exclusion() {
        let missing_baseline = ReleaseScanCompetitorCorpusMetadata {
            case_id: "release.scan_ac_irregular.1m",
            rule_family: "mixed-literal-regex",
            pattern_count: 128,
            literal_density_bps: 6_250,
            construct_classes: &["literal"],
            haystack_corpus_id: "heldout:scan:irregular:1m",
            baseline_engine: "",
            unsupported_construct_reasons: &[],
        };
        let error = validate_release_scan_competitor_corpus_metadata(&missing_baseline)
            .expect_err("Fix: missing scan baseline engine must reject");
        assert!(error.contains("baseline_engine"));

        let weak_exclusion = ReleaseScanCompetitorCorpusMetadata {
            baseline_engine: "hyperscan-compatible",
            unsupported_construct_reasons: &["look-around"],
            ..missing_baseline
        };
        let error = validate_release_scan_competitor_corpus_metadata(&weak_exclusion)
            .expect_err("Fix: weak unsupported construct reason must reject");
        assert!(error.contains("unsupported construct"));
    }

    #[test]
    fn release_macro_family_registry_preserves_exact_ids_owners_and_families() {
        let specs = release_macro_program_specs_for_records(33);
        let observed = specs
            .iter()
            .map(|spec| (spec.id, spec.owner_crate, spec.family, spec.input_buffers))
            .collect::<Vec<_>>();

        assert_eq!(
            observed,
            vec![
                (
                    "release.condition_eval.1m",
                    "vyre",
                    ReleaseMacroFamily::Condition,
                    3,
                ),
                (
                    "release.string_bitmap_scatter.1m",
                    "vyre-libs",
                    ReleaseMacroFamily::Scan,
                    2,
                ),
                (
                    "release.offset_count_aggregation.1m",
                    "vyre-libs",
                    ReleaseMacroFamily::Scan,
                    3,
                ),
                (
                    "release.entropy_window.1m",
                    "vyre-libs",
                    ReleaseMacroFamily::Scan,
                    3,
                ),
                (
                    "release.quantified_condition_loops.1m",
                    "vyre",
                    ReleaseMacroFamily::Condition,
                    3,
                ),
                (
                    "release.alias_reaching_def.1m",
                    "weir",
                    ReleaseMacroFamily::Flow,
                    3,
                ),
                (
                    "release.ifds_witness.1m",
                    "weir",
                    ReleaseMacroFamily::Flow,
                    3,
                ),
                (
                    "release.c_ast_traversal.1m",
                    "vyre-frontend-c",
                    ReleaseMacroFamily::Parser,
                    3,
                ),
                (
                    "release.megakernel_queue.1m",
                    "vyre-runtime",
                    ReleaseMacroFamily::Resident,
                    3,
                ),
                (
                    "release.egraph_saturation.1m",
                    "vyre-lower",
                    ReleaseMacroFamily::Egraph,
                    3,
                ),
            ],
            "Fix: typed release family registry must preserve the exact release macro case surface."
        );
    }

    #[test]
    fn release_macro_typed_family_builders_preserve_weir_flow_and_empty_families() {
        let flow_specs =
            release_macro_program_specs_for_family_and_records(ReleaseMacroFamily::Flow, 33);
        assert_eq!(
            flow_specs
                .iter()
                .map(|spec| (spec.id, spec.owner_crate, spec.family, spec.input_buffers))
                .collect::<Vec<_>>(),
            vec![
                (
                    "release.alias_reaching_def.1m",
                    "weir",
                    ReleaseMacroFamily::Flow,
                    3,
                ),
                (
                    "release.ifds_witness.1m",
                    "weir",
                    ReleaseMacroFamily::Flow,
                    3,
                ),
            ],
            "Fix: flow release workloads must stay attached to Weir ownership and not drift into benchmark-local flow."
        );

        let flow_cases =
            build_release_macro_cases_for_family_and_records(ReleaseMacroFamily::Flow, 33);
        assert_eq!(flow_cases.len(), flow_specs.len());
        for case in flow_cases {
            assert_eq!(case.input_bytes_total, 396);
            assert_eq!(
                case.expected_output_digest,
                digest64_buffers(&case.expected_outputs)
            );
            assert_ne!(
                case.expected_output_digest, 0,
                "Fix: Weir flow release case {} must carry a nonzero CPU-oracle digest.",
                case.spec.id
            );
        }

        for workload in release_macro_workloads_for_family(ReleaseMacroFamily::Flow) {
            assert!(
                workload.tags.contains(&"weir"),
                "Fix: Weir flow workload {} must advertise the Weir boundary in tags.",
                workload.id
            );
            assert!(
                workload.primitive.contains("Weir"),
                "Fix: Weir flow workload {} must name the actual Weir primitive.",
                workload.id
            );
        }

        assert!(
            release_macro_program_specs_for_family_and_records(ReleaseMacroFamily::Graph, 33)
                .is_empty()
        );
        assert!(
            build_release_macro_cases_for_family_and_records(ReleaseMacroFamily::Matrix, 33)
                .is_empty()
        );
    }

    #[test]
    fn release_macro_generated_cases_record_input_bytes_and_expected_output_digest() {
        for spec in release_macro_program_specs_for_records(33) {
            let case = build_release_macro_case_for_records(spec.id, spec.records)
                .expect("Fix: every release macro spec must build a generated case.");
            let expected_input_bytes = match spec.id {
                "release.string_bitmap_scatter.1m" => 272,
                _ => 396,
            };

            assert_eq!(case.spec.owner_crate, spec.owner_crate);
            assert_eq!(case.spec.family, spec.family);
            assert_eq!(case.input_bytes_total, expected_input_bytes);
            assert_eq!(
                case.expected_output_digest,
                digest64_buffers(&case.expected_outputs)
            );
            assert_ne!(
                case.expected_output_digest, 0,
                "Fix: generated release macro case {} must expose a nonzero expected output digest.",
                spec.id
            );
        }
    }
}

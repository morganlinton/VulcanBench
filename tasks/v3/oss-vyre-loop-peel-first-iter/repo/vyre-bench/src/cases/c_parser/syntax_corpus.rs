use super::corpus::linux_driver_corpus;
use super::support::{
    encode_parse_summary, metric, parse_summary_metric_points, require_encoded_syntax_surface,
    time_tree_sitter_c_corpus_baseline, time_tree_sitter_c_corpus_cold_baseline,
    tree_sitter_cold_speedup_metric, tree_sitter_speedup_metric, ParseSummaryMetricSurface,
};
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use vyre_frontend_c::api::{
    parse_prepared_resident_syntax, prepare_resident_syntax_bytes, CParseSummary,
    PreparedResidentSyntaxBytes,
};

pub(super) struct CParserSyntaxCorpusPipeline;
pub(super) struct CParserSyntaxCorpus100Pipeline;
pub(super) struct CParserSyntaxCorpus1000Pipeline;

struct CParserCorpusPrepared {
    source_len: usize,
    files: Vec<String>,
    resident: PreparedResidentSyntaxBytes,
}

fn prepare_syntax_corpus(workloads: usize) -> Result<CParserCorpusPrepared, BenchError> {
    let files = (0..workloads)
        .map(|_| linux_driver_corpus(1))
        .collect::<Vec<_>>();
    let source = files.join("\n");
    let source_len = source.len();
    let resident =
        prepare_resident_syntax_bytes(source.as_bytes()).map_err(BenchError::BackendFailed)?;
    Ok(CParserCorpusPrepared {
        source_len,
        files,
        resident,
    })
}

fn syntax_corpus_source_len(workloads: usize) -> u64 {
    (linux_driver_corpus(1).len() as u64)
        .saturating_mul(workloads as u64)
        .saturating_add(workloads.saturating_sub(1) as u64)
}

fn batch_syntax_summary(prepared: &CParserCorpusPrepared) -> Result<CParseSummary, BenchError> {
    let summary =
        parse_prepared_resident_syntax(&prepared.resident).map_err(BenchError::BackendFailed)?;
    Ok(CParseSummary {
        source_bytes: summary.source_bytes,
        token_count: summary.token_count,
        ast_bytes: summary.ast_bytes,
        ast_node_count: summary.ast_node_count,
        vast_bytes: 0,
        abi_layout_bytes: 0,
        expression_shape_bytes: 0,
        program_graph_bytes: 0,
        semantic_node_bytes: 0,
        semantic_edge_bytes: 0,
        sema_scope_bytes: 0,
        function_record_bytes: 0,
        call_record_bytes: 0,
    })
}

fn run_syntax_corpus_case(
    prepared: &CParserCorpusPrepared,
    label: &str,
) -> Result<BenchRun, BenchError> {
    let backend_acquire_ns = 0u64;
    let start = std::time::Instant::now();
    let summary = batch_syntax_summary(prepared)?;
    let wall_ns = start.elapsed().as_nanos() as u64;
    let source_refs = prepared
        .files
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let tree_sitter_timed = time_tree_sitter_c_corpus_baseline(&source_refs)?;
    let tree_sitter = tree_sitter_timed.baseline;
    let baseline_ns = tree_sitter_timed.wall_ns;
    let tree_sitter_cold = time_tree_sitter_c_corpus_cold_baseline(&source_refs)?;
    let vyre_cold_ns = backend_acquire_ns.saturating_add(wall_ns);
    let output = encode_parse_summary(summary);
    let mut custom = vec![
        metric("vyre_backend_acquire_ns", backend_acquire_ns),
        metric("vyre_cold_wall_ns", vyre_cold_ns),
        metric("tree_sitter_cold_wall_ns", tree_sitter_cold.wall_ns),
    ];
    custom.extend(parse_summary_metric_points(
        &summary,
        ParseSummaryMetricSurface::Full,
    ));
    custom.extend([
        metric("tree_sitter_c_ast_nodes", tree_sitter.nodes),
        metric("tree_sitter_c_has_error", u64::from(tree_sitter.has_error)),
        tree_sitter_speedup_metric(baseline_ns, wall_ns),
        tree_sitter_cold_speedup_metric(tree_sitter_cold.wall_ns, vyre_cold_ns),
    ]);
    Ok(BenchRun {
        metrics: BenchMetrics {
            wall_ns: Some(wall_ns),
            input_bytes: Some(prepared.source_len as u64),
            output_bytes: Some(output.len() as u64),
            bytes_touched: Some((prepared.source_len as u64).saturating_add(output.len() as u64)),
            custom,
            ..Default::default()
        },
        baseline_metrics: Some(BenchMetrics {
            wall_ns: Some(baseline_ns),
            input_bytes: Some(prepared.source_len as u64),
            output_bytes: Some(tree_sitter.nodes),
            bytes_touched: Some(prepared.source_len as u64),
            ..Default::default()
        }),
        outputs: vec![output],
        baseline_outputs: None,
    })
}

fn verify_syntax_corpus_case(run: &BenchRun, label: &str) -> Result<Correctness, BenchError> {
    let output = run.outputs.first().ok_or_else(|| {
        BenchError::CorrectnessViolation(format!("{label} benchmark produced no summary bytes"))
    })?;
    require_encoded_syntax_surface(output, label)?;
    Ok(Correctness::Certificate {
        digest: *blake3::hash(output).as_bytes(),
    })
}

fn corpus_bytes_touched(prepared: &PreparedCase) -> (u64, u64) {
    let bytes = prepared
        .downcast_ref::<CParserCorpusPrepared>()
        .map(|prepared| prepared.source_len as u64)
        .unwrap_or(0);
    (bytes, 0)
}

impl BenchCase for CParserSyntaxCorpusPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.syntax_only.linux_driver_corpus10".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Syntax Corpus 10".to_string(),
            description:
                "Vyre frontend C syntax-only GPU parser over ten Linux-driver-shaped workloads"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "syntax".to_string(),
                "c_ast".to_string(),
                "token".to_string(),
                "linux".to_string(),
                "corpus10".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-frontend-c".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        &[
            crate::api::suite::SuiteKind::Release,
            crate::api::suite::SuiteKind::Gpu,
            crate::api::suite::SuiteKind::Deep,
            crate::api::suite::SuiteKind::Honest,
        ]
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some(syntax_corpus_source_len(10)),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "linux-tu".to_string(),
                "syntax-only".to_string(),
                "corpus10".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux corpus10 syntax-only",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            10.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_syntax_corpus(10)?))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<CParserCorpusPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "C syntax corpus prepared payload type mismatch".to_string(),
                )
            })?;
        run_syntax_corpus_case(prepared, "C syntax corpus")
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        verify_syntax_corpus_case(run, "C syntax corpus")
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        corpus_bytes_touched(prepared)
    }
}

impl BenchCase for CParserSyntaxCorpus100Pipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.syntax_only.linux_driver_corpus100".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Syntax Corpus 100".to_string(),
            description:
                "Vyre frontend C syntax-only GPU parser over one hundred Linux-driver-shaped workloads"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "syntax".to_string(),
                "c_ast".to_string(),
                "token".to_string(),
                "linux".to_string(),
                "corpus100".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-frontend-c".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        &[
            crate::api::suite::SuiteKind::Release,
            crate::api::suite::SuiteKind::Gpu,
            crate::api::suite::SuiteKind::Deep,
            crate::api::suite::SuiteKind::Honest,
        ]
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some(syntax_corpus_source_len(100)),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "linux-tu".to_string(),
                "syntax-only".to_string(),
                "corpus100".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux corpus100 syntax-only",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            3.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_syntax_corpus(100)?))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<CParserCorpusPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "C syntax corpus100 prepared payload type mismatch".to_string(),
                )
            })?;
        run_syntax_corpus_case(prepared, "C syntax corpus100")
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        verify_syntax_corpus_case(run, "C syntax corpus100")
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        corpus_bytes_touched(prepared)
    }
}

impl BenchCase for CParserSyntaxCorpus1000Pipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.syntax_only.linux_driver_corpus1000".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Syntax Corpus 1000".to_string(),
            description:
                "Vyre frontend C syntax-only GPU parser over one thousand Linux-driver-shaped workloads"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "syntax".to_string(),
                "c_ast".to_string(),
                "token".to_string(),
                "linux".to_string(),
                "corpus1000".to_string(),
                "larger-kernel".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-frontend-c".to_string(),
        }
    }

    fn suites(&self) -> &'static [crate::api::suite::SuiteKind] {
        &[
            crate::api::suite::SuiteKind::Release,
            crate::api::suite::SuiteKind::Gpu,
            crate::api::suite::SuiteKind::Deep,
            crate::api::suite::SuiteKind::Honest,
        ]
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some(syntax_corpus_source_len(1000)),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "linux-tu".to_string(),
                "syntax-only".to_string(),
                "corpus1000".to_string(),
                "larger-kernel".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux corpus1000 syntax-only",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            10.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_syntax_corpus(1000)?))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<CParserCorpusPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "C syntax corpus1000 prepared payload type mismatch".to_string(),
                )
            })?;
        run_syntax_corpus_case(prepared, "C syntax corpus1000")
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        verify_syntax_corpus_case(run, "C syntax corpus1000")
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        corpus_bytes_touched(prepared)
    }
}

inventory::submit! {
    &CParserSyntaxCorpusPipeline as &'static dyn BenchCase
}

inventory::submit! {
    &CParserSyntaxCorpus100Pipeline as &'static dyn BenchCase
}

inventory::submit! {
    &CParserSyntaxCorpus1000Pipeline as &'static dyn BenchCase
}

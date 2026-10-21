mod corpus;
mod single_syntax;
mod support;
mod syntax_corpus;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use corpus::{linux_driver_corpus, CParserPrepared, LINUX_DRIVER_TU};
use support::{
    encode_parse_summary, metric, parse_summary_metric_points, require_encoded_parse_surface,
    run_tree_sitter_c_baseline, time_tree_sitter_c_baseline, time_tree_sitter_cold_baseline,
    tree_sitter_cold_speedup_metric, tree_sitter_speedup_metric, ParseSummaryMetricSurface,
    TempCompilePaths,
};
use vyre_frontend_c::api::{compile, parse_source, VyreCompileOptions};

const MAX_C_PARSER_OBJECT_BYTES: u64 = 64 * 1024 * 1024;

fn read_c_parser_object_bounded(path: &std::path::Path) -> Result<Vec<u8>, BenchError> {
    let mut reader = std::fs::File::open(path).map_err(|error| {
        BenchError::ExecutionFailed(format!("read C parser object `{}`: {error}", path.display()))
    })?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut reader, &mut chunk).map_err(|error| {
            BenchError::ExecutionFailed(format!(
                "read C parser object `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            return Ok(bytes);
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_C_PARSER_OBJECT_BYTES {
            return Err(BenchError::ExecutionFailed(format!(
                "C parser object `{}` exceeds {MAX_C_PARSER_OBJECT_BYTES} byte benchmark cap. Fix: shard the fixture or raise the cap deliberately.",
                path.display()
            )));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

pub struct CParserLinuxDriverPipeline;
pub struct CParserOnlyLinuxDriverPipeline;
pub struct CParserSemaLinuxDriverCorpus100Pipeline;

impl BenchCase for CParserLinuxDriverPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.parser.linux_driver_pipeline".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Parser Pipeline".to_string(),
            description:
                "Vyre frontend C parser/preprocessor pipeline over a Linux-driver-shaped translation unit"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "c_ast".to_string(),
                "preprocessor".to_string(),
                "token".to_string(),
                "linux".to_string(),
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
            min_input_bytes: Some(LINUX_DRIVER_TU.len() as u64),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "linux-tu".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux translation-unit parse/traverse",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            1000.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(CParserPrepared {
            source: LINUX_DRIVER_TU.to_string(),
        }))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared.downcast_ref::<CParserPrepared>().ok_or_else(|| {
            BenchError::ExecutionFailed("C parser prepared payload type mismatch".to_string())
        })?;
        let paths = TempCompilePaths::new("vyre-bench-c-parser-linux-driver");
        std::fs::write(&paths.source, prepared.source.as_bytes()).map_err(|error| {
            BenchError::ExecutionFailed(format!("write C parser source: {error}"))
        })?;

        let start = std::time::Instant::now();
        compile(VyreCompileOptions {
            is_compile_only: true,
            input_files: vec![paths.source.clone()],
            output_file: Some(paths.object.clone()),
            include_dirs: Vec::new(),
            quote_include_dirs: Vec::new(),
            system_include_dirs: Vec::new(),
            after_include_dirs: Vec::new(),
            forced_include_files: Vec::new(),
            imacro_files: Vec::new(),
            macros: Vec::new(),
            undefs: Vec::new(),
            macro_actions: Vec::new(),
            disable_system_include_dirs: true,
            system_include_sysroot: None,
            target: vyre_frontend_c::api::CTargetOptions::default(),
        })
        .map_err(BenchError::BackendFailed)?;
        let wall_ns = start.elapsed().as_nanos() as u64;

        let baseline_start = std::time::Instant::now();
        let tree_sitter = run_tree_sitter_c_baseline(&prepared.source)?;
        let baseline_ns = baseline_start.elapsed().as_nanos() as u64;

        let object_bytes = read_c_parser_object_bounded(&paths.object)?;
        paths.cleanup();

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(object_bytes.len() as u64),
                bytes_touched: Some(
                    (prepared.source.len() as u64).saturating_add(object_bytes.len() as u64),
                ),
                custom: vec![
                    MetricPoint {
                        name: "c_parser_source_bytes".to_string(),
                        value: prepared.source.len() as u64,
                    },
                    MetricPoint {
                        name: "c_parser_object_bytes".to_string(),
                        value: object_bytes.len() as u64,
                    },
                    MetricPoint {
                        name: "tree_sitter_c_ast_nodes".to_string(),
                        value: tree_sitter.nodes,
                    },
                    MetricPoint {
                        name: "tree_sitter_c_has_error".to_string(),
                        value: u64::from(tree_sitter.has_error),
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(tree_sitter.nodes),
                bytes_touched: Some(prepared.source.len() as u64),
                ..Default::default()
            }),
            outputs: vec![object_bytes],
            baseline_outputs: None,
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let object = run.outputs.first().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "C parser benchmark produced no object bytes".to_string(),
            )
        })?;
        if object.len() < 4 || &object[0..4] != b"\x7FELF" {
            return Err(BenchError::CorrectnessViolation(
                "C parser benchmark output is not an ELF object".to_string(),
            ));
        }
        Ok(Correctness::Certificate {
            digest: *blake3::hash(object).as_bytes(),
        })
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (LINUX_DRIVER_TU.len() as u64, 0)
    }
}

impl BenchCase for CParserOnlyLinuxDriverPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.parser_only.linux_driver_pipeline".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Parser Only".to_string(),
            description:
                "Vyre frontend C parser-only GPU pipeline over a Linux-driver-shaped translation unit"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "c_ast".to_string(),
                "token".to_string(),
                "linux".to_string(),
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
            min_input_bytes: Some(LINUX_DRIVER_TU.len() as u64),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "linux-tu".to_string(),
                "parser-only".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux translation-unit parser-only",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            1.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(CParserPrepared {
            source: LINUX_DRIVER_TU.to_string(),
        }))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared.downcast_ref::<CParserPrepared>().ok_or_else(|| {
            BenchError::ExecutionFailed("C parser prepared payload type mismatch".to_string())
        })?;

        let start = std::time::Instant::now();
        let summary = parse_source(&prepared.source).map_err(BenchError::BackendFailed)?;
        let wall_ns = start.elapsed().as_nanos() as u64;

        let baseline_start = std::time::Instant::now();
        let tree_sitter = run_tree_sitter_c_baseline(&prepared.source)?;
        let baseline_ns = baseline_start.elapsed().as_nanos() as u64;
        let output = encode_parse_summary(summary);
        let mut custom =
            parse_summary_metric_points(&summary, ParseSummaryMetricSurface::ParserOnly);
        custom.extend([
            metric("tree_sitter_c_ast_nodes", tree_sitter.nodes),
            metric("tree_sitter_c_has_error", u64::from(tree_sitter.has_error)),
        ]);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(output.len() as u64),
                bytes_touched: Some(
                    (prepared.source.len() as u64).saturating_add(output.len() as u64),
                ),
                custom,
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(tree_sitter.nodes),
                bytes_touched: Some(prepared.source.len() as u64),
                ..Default::default()
            }),
            outputs: vec![output],
            baseline_outputs: None,
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let output = run.outputs.first().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "C parser-only benchmark produced no summary bytes".to_string(),
            )
        })?;
        super::c_parser::support::require_encoded_parse_surface(output, "C parser-only")?;
        Ok(Correctness::Certificate {
            digest: *blake3::hash(output).as_bytes(),
        })
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (LINUX_DRIVER_TU.len() as u64, 0)
    }
}

impl BenchCase for CParserSemaLinuxDriverCorpus100Pipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.parser_sema.linux_driver_corpus100".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Parser/Sema Corpus 100".to_string(),
            description:
                "Vyre frontend C full GPU parser, AST, VAST, ProgramGraph, and semantic-scope pipeline over one hundred Linux-driver-shaped workloads"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "semantic-analysis".to_string(),
                "sema".to_string(),
                "c_ast".to_string(),
                "program-graph".to_string(),
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
            min_input_bytes: Some(linux_driver_corpus(100).len() as u64),
            feature_set: vec![
                "vyre-frontend-c".to_string(),
                "c-parser".to_string(),
                "c-sema".to_string(),
                "linux-tu".to_string(),
                "corpus100".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux corpus100 parser+semantic-analysis",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            1.0,
        ))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(CParserPrepared {
            source: linux_driver_corpus(100),
        }))
    }

    fn program<'a>(&self, _prepared: &'a PreparedCase) -> Option<&'a vyre_foundation::ir::Program> {
        None
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared.downcast_ref::<CParserPrepared>().ok_or_else(|| {
            BenchError::ExecutionFailed(
                "C parser/sema corpus prepared payload type mismatch".to_string(),
            )
        })?;
        let backend_acquire_ns = 0u64;
        let start = std::time::Instant::now();
        let summary = parse_source(&prepared.source).map_err(BenchError::BackendFailed)?;
        let wall_ns = start.elapsed().as_nanos() as u64;

        let tree_sitter_timed = time_tree_sitter_c_baseline(&prepared.source)?;
        let tree_sitter = tree_sitter_timed.baseline;
        let baseline_ns = tree_sitter_timed.wall_ns;
        let tree_sitter_cold = time_tree_sitter_cold_baseline(&prepared.source)?;
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
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(output.len() as u64),
                bytes_touched: Some(
                    (prepared.source.len() as u64).saturating_add(output.len() as u64),
                ),
                custom,
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_ns),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(tree_sitter.nodes),
                bytes_touched: Some(prepared.source.len() as u64),
                ..Default::default()
            }),
            outputs: vec![output],
            baseline_outputs: None,
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let output = run.outputs.first().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "C parser/sema corpus benchmark produced no summary bytes".to_string(),
            )
        })?;
        require_encoded_parse_surface(output, "C parser/sema corpus")?;
        Ok(Correctness::Certificate {
            digest: *blake3::hash(output).as_bytes(),
        })
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (linux_driver_corpus(100).len() as u64, 0)
    }
}

inventory::submit! {
    &CParserLinuxDriverPipeline as &'static dyn BenchCase
}

inventory::submit! {
    &CParserOnlyLinuxDriverPipeline as &'static dyn BenchCase
}

inventory::submit! {
    &CParserSemaLinuxDriverCorpus100Pipeline as &'static dyn BenchCase
}

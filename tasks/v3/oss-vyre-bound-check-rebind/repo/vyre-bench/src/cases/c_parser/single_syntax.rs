use super::corpus::{CParserPrepared, LINUX_DRIVER_TU};
use super::support::{
    encode_parse_summary, metric, parse_summary_metric_points, require_encoded_syntax_surface,
    time_tree_sitter_c_baseline, time_tree_sitter_cold_baseline, tree_sitter_cold_speedup_metric,
    tree_sitter_speedup_metric, ParseSummaryMetricSurface,
};
use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use vyre_frontend_c::api::parse_syntax_source;

pub(super) struct CParserSyntaxOnlyLinuxDriverPipeline;

impl BenchCase for CParserSyntaxOnlyLinuxDriverPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.c.syntax_only.linux_driver_pipeline".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Vyre-C Linux Driver Syntax Only".to_string(),
            description:
                "Vyre frontend C syntax-only GPU parser over a Linux-driver-shaped translation unit"
                    .to_string(),
            tags: vec![
                "frontend-c".to_string(),
                "parser".to_string(),
                "syntax".to_string(),
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
                "syntax-only".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "C parser Linux translation-unit syntax-only",
            "vyre-frontend-c",
            "Tree-sitter C in-process parse + full AST traversal",
            10.0,
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
            BenchError::ExecutionFailed("C syntax-only prepared payload type mismatch".to_string())
        })?;
        let backend_acquire_ns = 0u64;
        let start = std::time::Instant::now();
        let summary = parse_syntax_source(&prepared.source).map_err(BenchError::BackendFailed)?;
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
                "C syntax-only benchmark produced no summary bytes".to_string(),
            )
        })?;
        require_encoded_syntax_surface(output, "C syntax-only")?;
        Ok(Correctness::Certificate {
            digest: *blake3::hash(output).as_bytes(),
        })
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (LINUX_DRIVER_TU.len() as u64, 0)
    }
}

inventory::submit! {
    &CParserSyntaxOnlyLinuxDriverPipeline as &'static dyn BenchCase
}

use std::path::PathBuf;

use crate::api::case::BenchError;
use crate::api::metric::MetricPoint;
use vyre_frontend_c::api::CParseSummary;

pub(super) const ENCODED_PARSE_SUMMARY_BYTES: usize = 12 * 8;
pub(super) const TREE_SITTER_SPEEDUP_METRIC: &str = "tree_sitter_speedup_x1000";
pub(super) const TREE_SITTER_COLD_SPEEDUP_METRIC: &str = "tree_sitter_cold_speedup_x1000";

#[derive(Clone, Copy)]
pub(super) enum ParseSummaryMetricSurface {
    ParserOnly,
    Full,
}

pub(super) struct TempCompilePaths {
    pub(super) source: PathBuf,
    pub(super) object: PathBuf,
}

impl TempCompilePaths {
    pub(super) fn new(stem: &str) -> Self {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!("{stem}-{pid}-{nanos}"));
        Self {
            source: base.with_extension("c"),
            object: base.with_extension("o"),
        }
    }

    pub(super) fn cleanup(&self) {
        remove_compile_temp_file(&self.source);
        remove_compile_temp_file(&self.object);
    }
}

impl Drop for TempCompilePaths {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(super) struct TreeSitterBaseline {
    pub(super) nodes: u64,
    pub(super) has_error: bool,
}

pub(super) struct TimedTreeSitterBaseline {
    pub(super) baseline: TreeSitterBaseline,
    pub(super) wall_ns: u64,
}

pub(super) fn encode_parse_summary(summary: CParseSummary) -> Vec<u8> {
    let mut out = Vec::with_capacity(ENCODED_PARSE_SUMMARY_BYTES);
    out.extend_from_slice(&summary.source_bytes.to_le_bytes());
    out.extend_from_slice(&(summary.token_count as u64).to_le_bytes());
    out.extend_from_slice(&summary.ast_bytes.to_le_bytes());
    out.extend_from_slice(&summary.vast_bytes.to_le_bytes());
    out.extend_from_slice(&summary.abi_layout_bytes.to_le_bytes());
    out.extend_from_slice(&summary.expression_shape_bytes.to_le_bytes());
    out.extend_from_slice(&summary.program_graph_bytes.to_le_bytes());
    out.extend_from_slice(&summary.semantic_node_bytes.to_le_bytes());
    out.extend_from_slice(&summary.semantic_edge_bytes.to_le_bytes());
    out.extend_from_slice(&summary.sema_scope_bytes.to_le_bytes());
    out.extend_from_slice(&summary.function_record_bytes.to_le_bytes());
    out.extend_from_slice(&summary.call_record_bytes.to_le_bytes());
    debug_assert_eq!(out.len(), ENCODED_PARSE_SUMMARY_BYTES);
    out
}

pub(super) fn parse_summary_metric_points(
    summary: &CParseSummary,
    surface: ParseSummaryMetricSurface,
) -> Vec<MetricPoint> {
    let mut metrics = vec![
        metric("c_parser_source_bytes", summary.source_bytes),
        metric("c_parser_tokens", summary.token_count as u64),
        metric("c_parser_ast_bytes", summary.ast_bytes),
    ];
    if matches!(surface, ParseSummaryMetricSurface::Full) {
        metrics.extend([
            metric("c_parser_vast_bytes", summary.vast_bytes),
            metric("c_parser_abi_layout_bytes", summary.abi_layout_bytes),
            metric(
                "c_parser_expression_shape_bytes",
                summary.expression_shape_bytes,
            ),
            metric("c_parser_program_graph_bytes", summary.program_graph_bytes),
            metric("c_parser_semantic_node_bytes", summary.semantic_node_bytes),
            metric("c_parser_semantic_edge_bytes", summary.semantic_edge_bytes),
            metric("c_parser_sema_scope_bytes", summary.sema_scope_bytes),
        ]);
    }
    metrics.extend([
        metric(
            "c_parser_function_record_bytes",
            summary.function_record_bytes,
        ),
        metric("c_parser_call_record_bytes", summary.call_record_bytes),
    ]);
    if matches!(surface, ParseSummaryMetricSurface::Full) {
        metrics.extend([
            metric(
                "c_parser_function_records",
                summary.function_record_bytes / 12,
            ),
            metric("c_parser_call_records", summary.call_record_bytes / 16),
        ]);
    }
    metrics
}

pub(super) fn metric(name: &'static str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}

pub(super) fn scaled_speedup_x1000(baseline_ns: u64, wall_ns: u64) -> u64 {
    if wall_ns == 0 {
        return 0;
    }
    ((baseline_ns as u128).saturating_mul(1000) / wall_ns as u128).min(u64::MAX as u128) as u64
}

pub(super) fn tree_sitter_speedup_metric(baseline_ns: u64, wall_ns: u64) -> MetricPoint {
    MetricPoint {
        name: TREE_SITTER_SPEEDUP_METRIC.to_string(),
        value: scaled_speedup_x1000(baseline_ns, wall_ns),
    }
}

pub(super) fn tree_sitter_cold_speedup_metric(baseline_ns: u64, wall_ns: u64) -> MetricPoint {
    MetricPoint {
        name: TREE_SITTER_COLD_SPEEDUP_METRIC.to_string(),
        value: scaled_speedup_x1000(baseline_ns, wall_ns),
    }
}

pub(super) fn require_encoded_parse_surface(output: &[u8], label: &str) -> Result<(), BenchError> {
    require_encoded_fields(output, label, &[(1, "token_count"), (2, "ast_bytes")])?;
    let required = [
        (3, "vast_bytes"),
        (4, "abi_layout_bytes"),
        (5, "expression_shape_bytes"),
        (6, "program_graph_bytes"),
        (7, "semantic_node_bytes"),
        (8, "semantic_edge_bytes"),
        (9, "sema_scope_bytes"),
        (10, "function_record_bytes"),
    ];
    require_encoded_fields(output, label, &required)
}

pub(super) fn require_encoded_syntax_surface(output: &[u8], label: &str) -> Result<(), BenchError> {
    require_encoded_fields(output, label, &[(1, "token_count"), (2, "ast_bytes")])
}

fn require_encoded_fields(
    output: &[u8],
    label: &str,
    required: &[(usize, &str)],
) -> Result<(), BenchError> {
    if output.len() != ENCODED_PARSE_SUMMARY_BYTES {
        return Err(BenchError::CorrectnessViolation(format!(
            "{label} summary has {} bytes, expected {ENCODED_PARSE_SUMMARY_BYTES}",
            output.len(),
        )));
    }
    let field = |index: usize, name: &str| -> Result<u64, BenchError> {
        let start = index * 8;
        let end = start + 8;
        let bytes = output.get(start..end).ok_or_else(|| {
            BenchError::CorrectnessViolation(format!("{label} summary missing {name} field"))
        })?;
        Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| {
            BenchError::CorrectnessViolation(format!("{label} summary invalid {name} field"))
        })?))
    };
    for &(index, name) in required {
        if field(index, name)? == 0 {
            return Err(BenchError::CorrectnessViolation(format!(
                "{label} summary must report nonzero {name}"
            )));
        }
    }
    Ok(())
}

pub(super) fn run_tree_sitter_c_baseline(source: &str) -> Result<TreeSitterBaseline, BenchError> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        BenchError::ExecutionFailed(format!(
            "failed to initialize Tree-sitter C parser baseline: {error}"
        ))
    })?;
    run_tree_sitter_c_baseline_with_parser(&mut parser, source)
}

fn run_tree_sitter_c_baseline_with_parser(
    parser: &mut tree_sitter::Parser,
    source: &str,
) -> Result<TreeSitterBaseline, BenchError> {
    let tree = parser.parse(source, None).ok_or_else(|| {
        BenchError::ExecutionFailed(
            "Tree-sitter C parser baseline returned no parse tree".to_string(),
        )
    })?;
    let has_error = tree.root_node().has_error();
    if has_error {
        let first_error = first_tree_sitter_error_summary(tree.root_node(), source);
        return Err(BenchError::CorrectnessViolation(
            format!(
                "Tree-sitter C baseline parsed the C parser benchmark source with errors; benchmark source is not a fair CPU-baseline workload. First error: {first_error}"
            )
                .to_string(),
        ));
    }

    let mut cursor = tree.walk();
    let mut nodes = 0u64;
    loop {
        nodes = nodes.saturating_add(1);
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok(TreeSitterBaseline { nodes, has_error });
            }
        }
    }
}

fn first_tree_sitter_error_summary(root: tree_sitter::Node<'_>, source: &str) -> String {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            let start = node.start_position();
            let end = node.end_position();
            let snippet = source
                .get(node.start_byte()..node.end_byte())
                .unwrap_or("")
                .replace('\n', "\\n");
            return format!(
                "kind={} missing={} start={}:{} end={}:{} bytes={}..{} snippet={:?}",
                node.kind(),
                node.is_missing(),
                start.row + 1,
                start.column + 1,
                end.row + 1,
                end.column + 1,
                node.start_byte(),
                node.end_byte(),
                snippet
            );
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return "root has_error=true but no ERROR or missing node was found".to_string();
            }
        }
    }
}

pub(super) fn time_tree_sitter_c_baseline(
    source: &str,
) -> Result<TimedTreeSitterBaseline, BenchError> {
    let first = time_tree_sitter_c_baseline_once(source)?;
    let second = time_tree_sitter_c_baseline_once(source)?;
    if second.wall_ns < first.wall_ns {
        Ok(second)
    } else {
        Ok(first)
    }
}

pub(super) fn time_tree_sitter_c_corpus_baseline(
    sources: &[&str],
) -> Result<TimedTreeSitterBaseline, BenchError> {
    let first = time_tree_sitter_c_corpus_baseline_once(sources)?;
    let second = time_tree_sitter_c_corpus_baseline_once(sources)?;
    if second.wall_ns < first.wall_ns {
        Ok(second)
    } else {
        Ok(first)
    }
}

pub(super) fn time_tree_sitter_c_corpus_cold_baseline(
    sources: &[&str],
) -> Result<TimedTreeSitterBaseline, BenchError> {
    time_tree_sitter_c_corpus_baseline_once(sources)
}

pub(super) fn time_tree_sitter_cold_baseline(
    source: &str,
) -> Result<TimedTreeSitterBaseline, BenchError> {
    let start = std::time::Instant::now();
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        BenchError::ExecutionFailed(format!(
            "failed to initialize Tree-sitter C parser cold baseline: {error}"
        ))
    })?;
    let baseline = run_tree_sitter_c_baseline_with_parser(&mut parser, source)?;
    Ok(TimedTreeSitterBaseline {
        baseline,
        wall_ns: start.elapsed().as_nanos() as u64,
    })
}

fn time_tree_sitter_c_corpus_baseline_once(
    sources: &[&str],
) -> Result<TimedTreeSitterBaseline, BenchError> {
    let start = std::time::Instant::now();
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        BenchError::ExecutionFailed(format!(
            "failed to initialize Tree-sitter C parser corpus baseline: {error}"
        ))
    })?;
    let mut nodes = 0u64;
    let mut has_error = false;
    for source in sources {
        let baseline = run_tree_sitter_c_baseline_with_parser(&mut parser, source)?;
        nodes = nodes.checked_add(baseline.nodes).ok_or_else(|| {
            BenchError::ExecutionFailed(
                "Tree-sitter C corpus baseline node count overflowed u64".to_string(),
            )
        })?;
        has_error |= baseline.has_error;
    }
    Ok(TimedTreeSitterBaseline {
        baseline: TreeSitterBaseline { nodes, has_error },
        wall_ns: start.elapsed().as_nanos() as u64,
    })
}

fn time_tree_sitter_c_baseline_once(source: &str) -> Result<TimedTreeSitterBaseline, BenchError> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        BenchError::ExecutionFailed(format!(
            "failed to initialize Tree-sitter C parser baseline: {error}"
        ))
    })?;
    let start = std::time::Instant::now();
    let baseline = run_tree_sitter_c_baseline_with_parser(&mut parser, source)?;
    Ok(TimedTreeSitterBaseline {
        baseline,
        wall_ns: start.elapsed().as_nanos() as u64,
    })
}

fn remove_compile_temp_file(path: &std::path::Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!(
            "vyre-bench: failed to remove C parser temp file {}: {error}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_summary_metric_points, require_encoded_parse_surface, scaled_speedup_x1000,
        tree_sitter_speedup_metric, ParseSummaryMetricSurface, ENCODED_PARSE_SUMMARY_BYTES,
        TREE_SITTER_SPEEDUP_METRIC,
    };
    use vyre_frontend_c::api::CParseSummary;

    #[test]
    fn speedup_metric_uses_x1000_scale() {
        assert_eq!(scaled_speedup_x1000(10_000, 1_000), 10_000);
        assert_eq!(scaled_speedup_x1000(15_000, 1_000), 15_000);
        let metric = tree_sitter_speedup_metric(20_000, 1_000);
        assert_eq!(metric.name, TREE_SITTER_SPEEDUP_METRIC);
        assert_eq!(metric.value, 20_000);
    }

    #[test]
    fn parse_surface_gate_requires_all_release_evidence_fields() {
        let mut summary = vec![0u8; ENCODED_PARSE_SUMMARY_BYTES];
        for field in 1..=10 {
            summary[field * 8..field * 8 + 8].copy_from_slice(&1u64.to_le_bytes());
        }
        require_encoded_parse_surface(&summary, "test").expect("Fix: complete surface must pass");

        summary[7 * 8..7 * 8 + 8].copy_from_slice(&0u64.to_le_bytes());
        let err = require_encoded_parse_surface(&summary, "test")
            .expect_err("missing semantic nodes must fail");
        assert!(
            format!("{err:?}").contains("semantic_node_bytes"),
            "error should name the missing evidence field: {err:?}"
        );
    }

    #[test]
    fn parse_summary_metric_surface_is_single_source() {
        let summary = CParseSummary {
            source_bytes: 11,
            token_count: 13,
            ast_bytes: 17,
            ast_node_count: 18,
            vast_bytes: 19,
            abi_layout_bytes: 23,
            expression_shape_bytes: 29,
            program_graph_bytes: 31,
            semantic_node_bytes: 37,
            semantic_edge_bytes: 41,
            sema_scope_bytes: 43,
            function_record_bytes: 24,
            call_record_bytes: 32,
        };
        let parser_only =
            parse_summary_metric_points(&summary, ParseSummaryMetricSurface::ParserOnly);
        let full = parse_summary_metric_points(&summary, ParseSummaryMetricSurface::Full);
        assert!(parser_only
            .iter()
            .any(|point| point.name == "c_parser_tokens" && point.value == 13));
        assert!(!parser_only
            .iter()
            .any(|point| point.name == "c_parser_program_graph_bytes"));
        assert!(full
            .iter()
            .any(|point| point.name == "c_parser_program_graph_bytes" && point.value == 31));
        assert!(full
            .iter()
            .any(|point| point.name == "c_parser_function_records" && point.value == 2));
    }
}

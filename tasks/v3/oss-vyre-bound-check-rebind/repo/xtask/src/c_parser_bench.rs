//! CUDA C parser vs tree-sitter subsystem benchmark evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;
use walkdir::WalkDir;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::{
    parse_source, parse_syntax_bytes, CParseSummary, CliMacroAction, SyntaxParseSummary,
    VyreCompileOptions,
};
use vyre_frontend_c::object_format::{SectionTag, VYRECOB2_MAGIC};
use vyre_frontend_c::tu_host::prepare_resident_translation_unit_source_gpu;

static TEMP_PARSE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct CParserBenchBackend {
    backend_id: String,
    include_dirs: Vec<PathBuf>,
    macros: Vec<(String, Option<String>)>,
}

impl CParserBenchBackend {
    fn acquire(config: &Config) -> Result<Self, String> {
        let backend_id = vyre_frontend_c::pipeline::preferred_backend_id()?;
        match config.backend.as_str() {
            "preferred" => {}
            "cuda" if backend_id == "cuda" => {}
            "wgpu" if backend_id == "wgpu" => {}
            other => {
                return Err(format!(
                    "Fix: requested C parser backend `{other}` but frontend selected `{backend_id}`. The benchmark uses the production frontend backend selector; register the requested backend before running evidence."
                ));
            }
        }
        Ok(Self {
            backend_id,
            include_dirs: config.include_dirs.clone(),
            macros: config.macros.clone(),
        })
    }

    fn backend_id(&self) -> &str {
        &self.backend_id
    }

    fn compile_options_for(&self, input_file: PathBuf) -> VyreCompileOptions {
        VyreCompileOptions {
            is_compile_only: true,
            input_files: vec![input_file],
            output_file: None,
            include_dirs: self.include_dirs.clone(),
            quote_include_dirs: Vec::new(),
            system_include_dirs: Vec::new(),
            after_include_dirs: Vec::new(),
            forced_include_files: Vec::new(),
            imacro_files: Vec::new(),
            macros: Vec::new(),
            undefs: Vec::new(),
            macro_actions: define_actions(&self.macros),
            disable_system_include_dirs: false,
            system_include_sysroot: None,
            target: vyre_frontend_c::api::CTargetOptions::default(),
        }
    }
}

fn define_actions(macros: &[(String, Option<String>)]) -> Vec<CliMacroAction> {
    macros
        .iter()
        .map(|(name, value)| CliMacroAction::Define {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct ParserBenchReport {
    schema_version: u32,
    benchmark_protocol: &'static str,
    repro_command: String,
    input_representation: &'static str,
    timing_scope_policy: &'static str,
    source_io_timing_policy: &'static str,
    cuda_parser_input_policy: &'static str,
    host_repair_policy: &'static str,
    preprocessing_policy: &'static str,
    runtime_cpu_policy: &'static str,
    parser_lex_policy: &'static str,
    frontend_pipeline_cache_policy: &'static str,
    cuda_graph_policy: &'static str,
    c_frontend_fusion_policy: &'static str,
    tree_sitter_comparator_policy: &'static str,
    mode: &'static str,
    measurement_order: &'static str,
    corpus_root: String,
    corpus_root_canonical: String,
    file_count: usize,
    parsed_files: usize,
    failed_files: usize,
    total_source_bytes: u64,
    source_read_wall_ns: u128,
    batch_build_wall_ns: u128,
    batch_source_bytes: u64,
    batch_count: usize,
    max_batch_source_bytes: u64,
    requested_max_batch_bytes: usize,
    effective_max_batch_bytes: usize,
    batch_delimiter: &'static str,
    batch_delimiter_bytes: usize,
    warmup_count: usize,
    vyre_backend_id: String,
    vyre_init_ns: u128,
    tree_sitter_init_ns: u128,
    vyre_tokens: u32,
    vyre_ast_bytes: u64,
    vyre_vast_bytes: u64,
    vyre_abi_layout_bytes: u64,
    vyre_expression_shape_bytes: u64,
    vyre_program_graph_bytes: u64,
    vyre_semantic_node_bytes: u64,
    vyre_semantic_edge_bytes: u64,
    vyre_sema_scope_bytes: u64,
    vyre_tokens_per_mib_x1000: u128,
    vyre_ast_bytes_per_mib: u128,
    vyre_vast_bytes_per_mib: u128,
    vyre_abi_layout_bytes_per_mib: u128,
    vyre_semantic_graph_bytes_per_mib: u128,
    vyre_function_records_per_mib_x1000: u128,
    vyre_call_records_per_mib_x1000: u128,
    vyre_function_record_bytes: u64,
    vyre_call_record_bytes: u64,
    vyre_function_records: u64,
    vyre_call_records: u64,
    cold_vyre_wall_ns: u128,
    cold_tree_sitter_wall_ns: u128,
    cold_vyre_total_ns: u128,
    cold_tree_sitter_total_ns: u128,
    cold_speedup_x1000: u128,
    hot_vyre_wall_ns: u128,
    hot_tree_sitter_wall_ns: u128,
    hot_speedup_x1000: u128,
    reverse_order_vyre_wall_ns: u128,
    reverse_order_tree_sitter_wall_ns: u128,
    reverse_order_speedup_x1000: u128,
    vyre_wall_ns: u128,
    tree_sitter_wall_ns: u128,
    tree_sitter_has_error: bool,
    tree_sitter_error_batches: usize,
    tree_sitter_clean_batches: usize,
    tree_sitter_parse_failures: usize,
    per_file_tree_sitter_error_count: usize,
    release_evidence_valid: bool,
    speedup_x1000: u128,
    per_file_enabled: bool,
    per_file_vyre_enabled: bool,
    per_file_policy: &'static str,
    files: Vec<FileBench>,
    failures: Vec<FileFailure>,
}

#[derive(Debug, Serialize)]
struct FileBench {
    path: String,
    source_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_ast_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_vast_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_abi_layout_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_expression_shape_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_program_graph_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_semantic_node_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_semantic_edge_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_sema_scope_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_function_record_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_call_record_bytes: Option<u64>,
    vyre_function_records: Option<u64>,
    vyre_call_records: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vyre_wall_ns: Option<u128>,
    tree_sitter_wall_ns: u128,
    tree_sitter_has_error: bool,
}

#[derive(Debug, Serialize)]
struct FileFailure {
    path: String,
    source_bytes: u64,
    stage: String,
    error: String,
}

#[derive(Debug)]
struct SourceFile {
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct BenchBatch {
    bytes: Vec<u8>,
    file_start: usize,
    file_end: usize,
}

#[derive(Debug)]
struct Config {
    corpus: PathBuf,
    output: PathBuf,
    limit: Option<usize>,
    per_file: bool,
    warmups: usize,
    max_batch_bytes: usize,
    mode: BenchMode,
    order: BenchOrder,
    backend: String,
    include_dirs: Vec<PathBuf>,
    macros: Vec<(String, Option<String>)>,
    require_release_evidence: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BenchMode {
    Syntax,
    Parser,
}

impl BenchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Parser => "parser",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BenchOrder {
    VyreFirst,
    TreeSitterFirst,
    Both,
}

impl BenchOrder {
    fn as_str(self) -> &'static str {
        match self {
            Self::VyreFirst => "vyre-first",
            Self::TreeSitterFirst => "tree-sitter-first",
            Self::Both => "both",
        }
    }

    fn primary(self) -> Self {
        match self {
            Self::TreeSitterFirst => Self::TreeSitterFirst,
            Self::VyreFirst | Self::Both => Self::VyreFirst,
        }
    }

    fn reverse(self) -> Option<Self> {
        match self {
            Self::Both => Some(Self::TreeSitterFirst),
            Self::VyreFirst | Self::TreeSitterFirst => None,
        }
    }
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    match run_inner(&config) {
        Ok(report) => {
            if let Some(parent) = config.output.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    eprintln!(
                        "Fix: failed to create benchmark output directory `{}`: {error}",
                        parent.display()
                    );
                    std::process::exit(1);
                }
            }
            match serde_json::to_vec_pretty(&report) {
                Ok(bytes) => {
                    if let Err(error) = fs::write(&config.output, bytes) {
                        eprintln!(
                            "Fix: failed to write C parser benchmark report `{}`: {error}",
                            config.output.display()
                        );
                        std::process::exit(1);
                    }
                }
                Err(error) => {
                    eprintln!("Fix: failed to serialize C parser benchmark report: {error}");
                    std::process::exit(1);
                }
            }
            println!(
                "c-parser-bench: hot {:.3}x, cold-total {:.3}x over {} file(s), wrote {}",
                report.hot_speedup_x1000 as f64 / 1000.0,
                report.cold_speedup_x1000 as f64 / 1000.0,
                report.parsed_files,
                config.output.display()
            );
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_inner(config: &Config) -> Result<ParserBenchReport, String> {
    if !config.corpus.is_dir() {
        return Err(format!(
            "Fix: --corpus must be a directory, got `{}`.",
            config.corpus.display()
        ));
    }
    let mut paths = collect_c_files(&config.corpus)?;
    paths.sort();
    if let Some(limit) = config.limit {
        paths.truncate(limit);
    }
    if paths.is_empty() {
        return Err(format!(
            "Fix: parser benchmark corpus `{}` contains no .c files.",
            config.corpus.display()
        ));
    }

    let mut failures = Vec::new();
    let mut sources = Vec::with_capacity(paths.len());
    let mut total_source_bytes = 0u64;
    let source_read_start = Instant::now();
    for path in paths {
        match fs::read(&path) {
            Ok(bytes) => {
                total_source_bytes = total_source_bytes.saturating_add(bytes.len() as u64);
                sources.push(SourceFile { path, bytes });
            }
            Err(error) => failures.push(FileFailure {
                path: path.display().to_string(),
                source_bytes: 0,
                stage: "read".to_string(),
                error: error.to_string(),
            }),
        }
    }
    let source_read_wall_ns = source_read_start.elapsed().as_nanos();
    if sources.is_empty() {
        return Err(format!(
            "Fix: parser benchmark corpus `{}` had no readable .c files.",
            config.corpus.display()
        ));
    }

    let vyre_init_start = Instant::now();
    let vyre_backend = CParserBenchBackend::acquire(config)?;
    let vyre_backend_id = vyre_backend.backend_id().to_string();
    let vyre_init_ns = vyre_init_start.elapsed().as_nanos();
    let effective_max_batch_bytes = if vyre_backend_id == "wgpu" {
        config.max_batch_bytes.min(65_535usize * 256)
    } else {
        config.max_batch_bytes
    };

    let batch_build_start = Instant::now();
    let batch_sources = build_batch_sources(&sources, effective_max_batch_bytes);
    let batch_build_wall_ns = batch_build_start.elapsed().as_nanos();
    let corpus_root_canonical = config
        .corpus
        .canonicalize()
        .unwrap_or_else(|_| config.corpus.clone());

    let tree_sitter_init_start = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|error| format!("Fix: failed to initialize tree-sitter-c: {error}"))?;
    let tree_sitter_init_ns = tree_sitter_init_start.elapsed().as_nanos();

    let mut cold = BatchPairTotals::default();
    let mut hot = BatchPairTotals::default();
    let mut reverse = BatchPairTotals::default();
    let mut tree_sitter_has_error = false;
    let mut tree_sitter_error_batches = 0usize;
    let mut tree_sitter_clean_batches = 0usize;
    let tree_sitter_parse_failures = 0usize;
    let mut batch_source_bytes = 0u64;
    let mut max_batch_source_bytes = 0u64;
    let primary_order = config.order.primary();
    for batch in &batch_sources {
        batch_source_bytes = batch_source_bytes.saturating_add(batch.bytes.len() as u64);
        max_batch_source_bytes = max_batch_source_bytes.max(batch.bytes.len() as u64);

        let cold_batch = run_batch_pair(
            primary_order,
            config.mode,
            &vyre_backend,
            batch,
            &sources,
            &mut parser,
            config.require_release_evidence,
        )?;
        tree_sitter_has_error |= cold_batch.tree_sitter_has_error;
        if cold_batch.tree_sitter_has_error {
            tree_sitter_error_batches = tree_sitter_error_batches.saturating_add(1);
        } else {
            tree_sitter_clean_batches = tree_sitter_clean_batches.saturating_add(1);
        }
        cold.add(cold_batch);

        for _ in 0..config.warmups {
            run_vyre_batch(config.mode, &vyre_backend, batch, &sources).map_err(|error| {
                format!(
                    "Fix: vyre failed during hot warmup {} parse: {error}",
                    config.mode.as_str()
                )
            })?;
            run_tree_sitter_comparator(
                batch,
                &sources,
                &mut parser,
                config.require_release_evidence,
            )
            .map_err(|error| format!("Fix: tree-sitter failed during hot warmup parse: {error}"))?;
        }

        let hot_batch = run_batch_pair(
            primary_order,
            config.mode,
            &vyre_backend,
            batch,
            &sources,
            &mut parser,
            config.require_release_evidence,
        )?;
        tree_sitter_has_error |= hot_batch.tree_sitter_has_error;
        if hot_batch.tree_sitter_has_error {
            tree_sitter_error_batches = tree_sitter_error_batches.saturating_add(1);
        } else {
            tree_sitter_clean_batches = tree_sitter_clean_batches.saturating_add(1);
        }
        hot.add(hot_batch);

        if let Some(reverse_order) = config.order.reverse() {
            let reverse_batch = run_batch_pair(
                reverse_order,
                config.mode,
                &vyre_backend,
                batch,
                &sources,
                &mut parser,
                config.require_release_evidence,
            )?;
            tree_sitter_has_error |= reverse_batch.tree_sitter_has_error;
            if reverse_batch.tree_sitter_has_error {
                tree_sitter_error_batches = tree_sitter_error_batches.saturating_add(1);
            } else {
                tree_sitter_clean_batches = tree_sitter_clean_batches.saturating_add(1);
            }
            reverse.add(reverse_batch);
        }
    }
    let cold_vyre_total_ns = vyre_init_ns.saturating_add(cold.vyre_wall_ns);
    let cold_tree_sitter_total_ns = tree_sitter_init_ns.saturating_add(cold.tree_sitter_wall_ns);
    let cold_speedup_x1000 = speedup_x1000(cold_tree_sitter_total_ns, cold_vyre_total_ns);
    let hot_speedup_x1000 = speedup_x1000(hot.tree_sitter_wall_ns, hot.vyre_wall_ns);
    let reverse_order_speedup_x1000 =
        speedup_x1000(reverse.tree_sitter_wall_ns, reverse.vyre_wall_ns);
    if cold.vyre != hot.vyre {
        return Err(format!(
            "Fix: vyre parser evidence is nondeterministic between cold and hot runs: cold={:?}, hot={:?}.",
            cold.vyre, hot.vyre
        ));
    }
    if config.order == BenchOrder::Both && reverse.vyre != hot.vyre {
        return Err(format!(
            "Fix: vyre parser evidence is order-sensitive: hot={:?}, reverse={:?}.",
            hot.vyre, reverse.vyre
        ));
    }

    if total_source_bytes > 0 && hot.vyre.token_count == 0 {
        return Err(format!(
            "Fix: vyre parser emitted zero tokens for a non-empty corpus on backend `{}`; refusing to record invalid speed evidence.",
            vyre_backend_id
        ));
    }
    if total_source_bytes > 0 && hot.vyre.ast_bytes == 0 {
        return Err(format!(
            "Fix: vyre parser emitted zero AST bytes for a non-empty corpus on backend `{}`; refusing to record invalid speed evidence.",
            vyre_backend_id
        ));
    }

    let files = if config.per_file {
        run_per_file_benchmarks(
            config.mode,
            &vyre_backend,
            &sources,
            &mut parser,
            &mut failures,
            !config.require_release_evidence,
        )?
    } else {
        Vec::new()
    };
    let per_file_tree_sitter_error_count = files
        .iter()
        .filter(|file| file.tree_sitter_has_error)
        .count();
    let release_evidence_valid = is_release_evidence_valid(
        is_full_linux_corpus(&corpus_root_canonical),
        sources.len() + failures.len(),
        failures.len(),
        tree_sitter_has_error,
        tree_sitter_error_batches,
        tree_sitter_parse_failures,
        per_file_tree_sitter_error_count,
        config.per_file,
        files.len(),
        hot.vyre.token_count,
        hot.vyre.ast_bytes,
        hot.vyre.vast_bytes,
        hot.vyre.abi_layout_bytes,
        hot.vyre.expression_shape_bytes,
        hot.vyre.program_graph_bytes,
        hot.vyre.semantic_node_bytes,
        hot.vyre.semantic_edge_bytes,
        hot.vyre.sema_scope_bytes,
        hot.vyre.function_record_bytes,
        hot.vyre.call_record_bytes,
        hot.vyre.function_record_bytes / 12,
        hot.vyre.call_record_bytes / 16,
        cold_speedup_x1000,
        hot_speedup_x1000,
        reverse_order_speedup_x1000,
        config.order,
    );
    if config.require_release_evidence && !release_evidence_valid {
        return Err(format!(
            "Fix: release parser evidence gate failed for backend `{}`: file_count={}, failures={}, tree_sitter_has_error={}, tree_sitter_error_batches={}, tree_sitter_parse_failures={}, per_file_enabled={}, per_file_entries={}, per_file_tree_sitter_error_count={}, cold_speedup_x={:.3}, hot_speedup_x={:.3}, reverse_speedup_x={:.3}, vyre_tokens={}, vyre_ast_bytes={}, vyre_vast_bytes={}, vyre_abi_layout_bytes={}, vyre_expression_shape_bytes={}, vyre_program_graph_bytes={}, vyre_semantic_node_bytes={}, vyre_semantic_edge_bytes={}, vyre_sema_scope_bytes={}, vyre_function_record_bytes={}, vyre_call_record_bytes={}, vyre_function_records={}, vyre_call_records={}.",
            vyre_backend_id,
            sources.len() + failures.len(),
            failures.len(),
            tree_sitter_has_error,
            tree_sitter_error_batches,
            tree_sitter_parse_failures,
            config.per_file,
            files.len(),
            per_file_tree_sitter_error_count,
            cold_speedup_x1000 as f64 / 1000.0,
            hot_speedup_x1000 as f64 / 1000.0,
            reverse_order_speedup_x1000 as f64 / 1000.0,
            hot.vyre.token_count,
            hot.vyre.ast_bytes,
            hot.vyre.vast_bytes,
            hot.vyre.abi_layout_bytes,
            hot.vyre.expression_shape_bytes,
            hot.vyre.program_graph_bytes,
            hot.vyre.semantic_node_bytes,
            hot.vyre.semantic_edge_bytes,
            hot.vyre.sema_scope_bytes,
            hot.vyre.function_record_bytes,
            hot.vyre.call_record_bytes,
            hot.vyre.function_record_bytes / 12,
            hot.vyre.call_record_bytes / 16
        ));
    }

    Ok(ParserBenchReport {
        schema_version: 32,
        benchmark_protocol: "preloaded-source/cold-first-touch-plus-hot/order-aware",
        repro_command: repro_command(config),
        input_representation: "raw-bytes",
        timing_scope_policy: "raw-parser-call-after-identical-source-preload",
        source_io_timing_policy: "reported-not-in-speedup-shared-input-preload",
        cuda_parser_input_policy: "packed-byte-megakernel-haystack",
        host_repair_policy: "none",
        preprocessing_policy: "gpu-resident-directive-eval-explicit-host-fs-only",
        runtime_cpu_policy: "forbidden-reference-oracle-explicit-conformance-only",
        parser_lex_policy: "cuda-megakernel-all-sizes-keyword-fused-wgpu-full-single-pass-no-host-lex-gate",
        frontend_pipeline_cache_policy: "default-on-disable-env-forbidden-release",
        cuda_graph_policy: "default-on-disable-env-forbidden-release",
        c_frontend_fusion_policy: "measured-selective-fusion-disabled-by-default-because-current-cuda-fused-vast-is-slower",
        tree_sitter_comparator_policy: if config.require_release_evidence {
            "clean-per-file-same-batch-file-range"
        } else {
            "batched-concatenated-source"
        },
        mode: config.mode.as_str(),
        measurement_order: config.order.as_str(),
        corpus_root: config.corpus.display().to_string(),
        corpus_root_canonical: corpus_root_canonical.display().to_string(),
        file_count: sources.len() + failures.len(),
        parsed_files: sources.len(),
        failed_files: failures.len(),
        total_source_bytes,
        source_read_wall_ns,
        batch_build_wall_ns,
        batch_source_bytes,
        batch_count: batch_sources.len(),
        max_batch_source_bytes,
        requested_max_batch_bytes: config.max_batch_bytes,
        effective_max_batch_bytes,
        batch_delimiter: "\\n;\\n",
        batch_delimiter_bytes: 3,
        warmup_count: config.warmups,
        vyre_backend_id,
        vyre_init_ns,
        tree_sitter_init_ns,
        vyre_tokens: hot.vyre.token_count,
        vyre_ast_bytes: hot.vyre.ast_bytes,
        vyre_vast_bytes: hot.vyre.vast_bytes,
        vyre_abi_layout_bytes: hot.vyre.abi_layout_bytes,
        vyre_expression_shape_bytes: hot.vyre.expression_shape_bytes,
        vyre_program_graph_bytes: hot.vyre.program_graph_bytes,
        vyre_semantic_node_bytes: hot.vyre.semantic_node_bytes,
        vyre_semantic_edge_bytes: hot.vyre.semantic_edge_bytes,
        vyre_sema_scope_bytes: hot.vyre.sema_scope_bytes,
        vyre_tokens_per_mib_x1000: density_per_mib_x1000(
            u128::from(hot.vyre.token_count),
            batch_source_bytes,
        ),
        vyre_ast_bytes_per_mib: density_per_mib(
            u128::from(hot.vyre.ast_bytes),
            batch_source_bytes,
        ),
        vyre_vast_bytes_per_mib: density_per_mib(
            u128::from(hot.vyre.vast_bytes),
            batch_source_bytes,
        ),
        vyre_abi_layout_bytes_per_mib: density_per_mib(
            u128::from(hot.vyre.abi_layout_bytes),
            batch_source_bytes,
        ),
        vyre_semantic_graph_bytes_per_mib: density_per_mib(
            u128::from(
                hot.vyre
                    .semantic_node_bytes
                    .saturating_add(hot.vyre.semantic_edge_bytes),
            ),
            batch_source_bytes,
        ),
        vyre_function_record_bytes: hot.vyre.function_record_bytes,
        vyre_call_record_bytes: hot.vyre.call_record_bytes,
        vyre_function_records: hot.vyre.function_record_bytes / 12,
        vyre_call_records: hot.vyre.call_record_bytes / 16,
        vyre_function_records_per_mib_x1000: density_per_mib_x1000(
            u128::from(hot.vyre.function_record_bytes / 12),
            batch_source_bytes,
        ),
        vyre_call_records_per_mib_x1000: density_per_mib_x1000(
            u128::from(hot.vyre.call_record_bytes / 16),
            batch_source_bytes,
        ),
        cold_vyre_wall_ns: cold.vyre_wall_ns,
        cold_tree_sitter_wall_ns: cold.tree_sitter_wall_ns,
        cold_vyre_total_ns,
        cold_tree_sitter_total_ns,
        cold_speedup_x1000,
        hot_vyre_wall_ns: hot.vyre_wall_ns,
        hot_tree_sitter_wall_ns: hot.tree_sitter_wall_ns,
        hot_speedup_x1000,
        reverse_order_vyre_wall_ns: reverse.vyre_wall_ns,
        reverse_order_tree_sitter_wall_ns: reverse.tree_sitter_wall_ns,
        reverse_order_speedup_x1000,
        vyre_wall_ns: hot.vyre_wall_ns,
        tree_sitter_wall_ns: hot.tree_sitter_wall_ns,
        tree_sitter_has_error,
        tree_sitter_error_batches,
        tree_sitter_clean_batches,
        tree_sitter_parse_failures,
        per_file_tree_sitter_error_count,
        release_evidence_valid,
        speedup_x1000: hot_speedup_x1000,
        per_file_enabled: config.per_file,
        per_file_vyre_enabled: config.per_file && !config.require_release_evidence,
        per_file_policy: if config.require_release_evidence {
            "tree-sitter-cleanliness-only"
        } else {
            "vyre-and-tree-sitter-diagnostic-timing"
        },
        files,
        failures,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]

struct VyreBenchSummary {
    token_count: u32,
    ast_bytes: u64,
    vast_bytes: u64,
    abi_layout_bytes: u64,
    expression_shape_bytes: u64,
    program_graph_bytes: u64,
    semantic_node_bytes: u64,
    semantic_edge_bytes: u64,
    sema_scope_bytes: u64,
    function_record_bytes: u64,
    call_record_bytes: u64,
}

impl VyreBenchSummary {
    fn add(self, other: Self) -> Self {
        Self {
            token_count: self.token_count.saturating_add(other.token_count),
            ast_bytes: self.ast_bytes.saturating_add(other.ast_bytes),
            vast_bytes: self.vast_bytes.saturating_add(other.vast_bytes),
            abi_layout_bytes: self.abi_layout_bytes.saturating_add(other.abi_layout_bytes),
            expression_shape_bytes: self
                .expression_shape_bytes
                .saturating_add(other.expression_shape_bytes),
            program_graph_bytes: self
                .program_graph_bytes
                .saturating_add(other.program_graph_bytes),
            semantic_node_bytes: self
                .semantic_node_bytes
                .saturating_add(other.semantic_node_bytes),
            semantic_edge_bytes: self
                .semantic_edge_bytes
                .saturating_add(other.semantic_edge_bytes),
            sema_scope_bytes: self.sema_scope_bytes.saturating_add(other.sema_scope_bytes),
            function_record_bytes: self
                .function_record_bytes
                .saturating_add(other.function_record_bytes),
            call_record_bytes: self
                .call_record_bytes
                .saturating_add(other.call_record_bytes),
        }
    }
}

impl Default for VyreBenchSummary {
    fn default() -> Self {
        Self {
            token_count: 0,
            ast_bytes: 0,
            vast_bytes: 0,
            abi_layout_bytes: 0,
            expression_shape_bytes: 0,
            program_graph_bytes: 0,
            semantic_node_bytes: 0,
            semantic_edge_bytes: 0,
            sema_scope_bytes: 0,
            function_record_bytes: 0,
            call_record_bytes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BatchPairRun {
    vyre: VyreBenchSummary,
    vyre_wall_ns: u128,
    tree_sitter_wall_ns: u128,
    tree_sitter_has_error: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct BatchPairTotals {
    vyre: VyreBenchSummary,
    vyre_wall_ns: u128,
    tree_sitter_wall_ns: u128,
}

impl BatchPairTotals {
    fn add(&mut self, run: BatchPairRun) {
        self.vyre = self.vyre.add(run.vyre);
        self.vyre_wall_ns = self.vyre_wall_ns.saturating_add(run.vyre_wall_ns);
        self.tree_sitter_wall_ns = self
            .tree_sitter_wall_ns
            .saturating_add(run.tree_sitter_wall_ns);
    }
}

fn speedup_x1000(tree_sitter_ns: u128, vyre_ns: u128) -> u128 {
    if vyre_ns == 0 {
        0
    } else {
        tree_sitter_ns.saturating_mul(1000) / vyre_ns
    }
}

fn density_per_mib_x1000(value: u128, bytes: u64) -> u128 {
    if bytes == 0 {
        0
    } else {
        value
            .saturating_mul(1024)
            .saturating_mul(1024)
            .saturating_mul(1000)
            / u128::from(bytes)
    }
}

fn density_per_mib(value: u128, bytes: u64) -> u128 {
    density_per_mib_x1000(value, bytes) / 1000
}

fn repro_command(config: &Config) -> String {
    let mut parts = vec![
        "cargo_full".to_string(),
        "run".to_string(),
        "--release".to_string(),
        "--bin".to_string(),
        "xtask".to_string(),
        "--".to_string(),
        "c-parser-bench".to_string(),
        "--corpus".to_string(),
        shell_word(&config.corpus.display().to_string()),
        "--output".to_string(),
        shell_word(&config.output.display().to_string()),
        "--warmups".to_string(),
        config.warmups.to_string(),
        "--mode".to_string(),
        config.mode.as_str().to_string(),
        "--order".to_string(),
        config.order.as_str().to_string(),
        "--backend".to_string(),
        config.backend.clone(),
        "--max-batch-bytes".to_string(),
        config.max_batch_bytes.to_string(),
    ];
    for include in &config.include_dirs {
        parts.push("-I".to_string());
        parts.push(shell_word(&include.display().to_string()));
    }
    for (name, value) in &config.macros {
        parts.push("-D".to_string());
        parts.push(shell_word(&match value {
            Some(value) => format!("{name}={value}"),
            None => name.clone(),
        }));
    }
    if let Some(limit) = config.limit {
        parts.push("--limit".to_string());
        parts.push(limit.to_string());
    }
    if config.per_file {
        parts.push("--per-file".to_string());
    }
    if config.require_release_evidence {
        parts.push("--require-release-evidence".to_string());
    }
    parts.join(" ")
}

fn shell_word(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn is_release_evidence_valid(
    full_linux_corpus: bool,
    file_count: usize,
    failure_count: usize,
    tree_sitter_has_error: bool,
    tree_sitter_error_batches: usize,
    tree_sitter_parse_failures: usize,
    per_file_tree_sitter_error_count: usize,
    per_file_enabled: bool,
    per_file_entry_count: usize,
    vyre_tokens: u32,
    vyre_ast_bytes: u64,
    vyre_vast_bytes: u64,
    vyre_abi_layout_bytes: u64,
    vyre_expression_shape_bytes: u64,
    vyre_program_graph_bytes: u64,
    vyre_semantic_node_bytes: u64,
    vyre_semantic_edge_bytes: u64,
    vyre_sema_scope_bytes: u64,
    vyre_function_record_bytes: u64,
    vyre_call_record_bytes: u64,
    vyre_function_records: u64,
    vyre_call_records: u64,
    cold_speedup_x1000: u128,
    hot_speedup_x1000: u128,
    reverse_order_speedup_x1000: u128,
    order: BenchOrder,
) -> bool {
    full_linux_corpus
        && file_count >= 30_000
        && failure_count == 0
        && !tree_sitter_has_error
        && tree_sitter_error_batches == 0
        && tree_sitter_parse_failures == 0
        && per_file_enabled
        && per_file_entry_count == file_count
        && per_file_tree_sitter_error_count == 0
        && vyre_tokens > 0
        && vyre_ast_bytes > 0
        && vyre_vast_bytes > 0
        && vyre_abi_layout_bytes > 0
        && vyre_expression_shape_bytes > 0
        && vyre_program_graph_bytes > 0
        && vyre_semantic_node_bytes > 0
        && vyre_semantic_edge_bytes > 0
        && vyre_sema_scope_bytes > 0
        && vyre_function_record_bytes > 0
        && vyre_call_record_bytes > 0
        && vyre_function_records > 0
        && vyre_call_records > 0
        && cold_speedup_x1000 >= 11_000
        && hot_speedup_x1000 >= 11_000
        && (order != BenchOrder::Both || reverse_order_speedup_x1000 >= 11_000)
}

fn is_full_linux_corpus(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("linux")
}

fn run_batch_pair(
    order: BenchOrder,
    mode: BenchMode,
    backend: &CParserBenchBackend,
    batch: &BenchBatch,
    sources: &[SourceFile],
    parser: &mut tree_sitter::Parser,
    clean_tree_sitter_per_file: bool,
) -> Result<BatchPairRun, String> {
    match order {
        BenchOrder::VyreFirst => {
            let (vyre, vyre_wall_ns) = time_vyre_batch(mode, backend, batch, sources)?;
            let (tree_sitter_wall_ns, tree_sitter_has_error) =
                run_tree_sitter_comparator(batch, sources, parser, clean_tree_sitter_per_file)?;
            Ok(BatchPairRun {
                vyre,
                vyre_wall_ns,
                tree_sitter_wall_ns,
                tree_sitter_has_error,
            })
        }
        BenchOrder::TreeSitterFirst => {
            let (tree_sitter_wall_ns, tree_sitter_has_error) =
                run_tree_sitter_comparator(batch, sources, parser, clean_tree_sitter_per_file)?;
            let (vyre, vyre_wall_ns) = time_vyre_batch(mode, backend, batch, sources)?;
            Ok(BatchPairRun {
                vyre,
                vyre_wall_ns,
                tree_sitter_wall_ns,
                tree_sitter_has_error,
            })
        }
        BenchOrder::Both => unreachable!("BenchOrder::Both is expanded before timed execution"),
    }
}

fn time_vyre_batch(
    mode: BenchMode,
    backend: &CParserBenchBackend,
    batch: &BenchBatch,
    sources: &[SourceFile],
) -> Result<(VyreBenchSummary, u128), String> {
    let vyre_start = Instant::now();
    let summary = run_vyre_batch(mode, backend, batch, sources).map_err(|error| {
        format!(
            "Fix: vyre failed to parse batched C corpus in {} mode: {error}",
            mode.as_str()
        )
    })?;
    Ok((summary, vyre_start.elapsed().as_nanos()))
}

fn run_tree_sitter_batch(
    batch_source: &[u8],
    parser: &mut tree_sitter::Parser,
) -> Result<(u128, bool), String> {
    let tree_sitter_start = Instant::now();
    let tree = parser.parse(batch_source, None).ok_or_else(|| {
        "Fix: tree-sitter returned no parse tree for batched C corpus.".to_string()
    })?;
    Ok((
        tree_sitter_start.elapsed().as_nanos(),
        tree.root_node().has_error(),
    ))
}

fn run_tree_sitter_comparator(
    batch: &BenchBatch,
    sources: &[SourceFile],
    parser: &mut tree_sitter::Parser,
    clean_per_file: bool,
) -> Result<(u128, bool), String> {
    if clean_per_file {
        run_tree_sitter_file_range(sources, batch.file_start, batch.file_end, parser)
    } else {
        run_tree_sitter_batch(&batch.bytes, parser)
    }
}

fn run_tree_sitter_file_range(
    sources: &[SourceFile],
    start: usize,
    end: usize,
    parser: &mut tree_sitter::Parser,
) -> Result<(u128, bool), String> {
    let tree_sitter_start = Instant::now();
    let mut has_error = false;
    for source in sources.get(start..end).ok_or_else(|| {
        format!(
            "Fix: tree-sitter file-range comparator received invalid range [{start}, {end}) for {} sources.",
            sources.len()
        )
    })? {
        let tree = parser.parse(&source.bytes, None).ok_or_else(|| {
            format!(
                "Fix: tree-sitter returned no parse tree for `{}`.",
                source.path.display()
            )
        })?;
        has_error |= tree.root_node().has_error();
    }
    Ok((tree_sitter_start.elapsed().as_nanos(), has_error))
}

fn run_vyre_batch(
    mode: BenchMode,
    backend: &CParserBenchBackend,
    batch: &BenchBatch,
    sources: &[SourceFile],
) -> Result<VyreBenchSummary, String> {
    match mode {
        BenchMode::Syntax => parse_syntax_bytes(&batch.bytes).map(summary_from_syntax_parse),
        BenchMode::Parser => run_vyre_parser_file_range(backend, batch, sources),
    }
}

fn run_vyre_file(
    mode: BenchMode,
    backend: &CParserBenchBackend,
    source: &SourceFile,
) -> Result<VyreBenchSummary, String> {
    match mode {
        BenchMode::Syntax => parse_syntax_bytes(&source.bytes).map(summary_from_syntax_parse),
        BenchMode::Parser => parse_full_gpu_source_file(source, backend),
    }
}

fn run_vyre_parser_file_range(
    backend: &CParserBenchBackend,
    batch: &BenchBatch,
    sources: &[SourceFile],
) -> Result<VyreBenchSummary, String> {
    let file_range = sources
        .get(batch.file_start..batch.file_end)
        .ok_or_else(|| {
            format!(
                "Fix: vyre parser benchmark batch range [{}..{}) exceeds {} source files.",
                batch.file_start,
                batch.file_end,
                sources.len()
            )
        })?;
    let mut resident_batch = String::with_capacity(batch.bytes.len());
    for source in file_range {
        let raw = parser_bench_source_text(source)?;
        let options = backend.compile_options_for(source.path.clone());
        let resident = prepare_resident_translation_unit_source_gpu(&source.path, &raw, &options)
            .map_err(|error| {
            format!(
                "Fix: resident GPU preprocessing failed for `{}`: {error}",
                source.path.display()
            )
        })?;
        resident_batch.push_str(&resident);
        if !resident_batch.ends_with('\n') {
            resident_batch.push('\n');
        }
        resident_batch.push_str(";\n");
    }
    let summary = summary_from_c_parse(parse_source(&resident_batch)?);
    if !resident_batch.is_empty() {
        require_nonzero_full_parser_section(summary.ast_bytes, "AST")?;
        require_nonzero_full_parser_section(summary.vast_bytes, "VAST")?;
        require_nonzero_full_parser_section(summary.abi_layout_bytes, "ABI layout")?;
        require_nonzero_full_parser_section(summary.expression_shape_bytes, "expression shape")?;
        require_nonzero_full_parser_section(summary.program_graph_bytes, "program graph")?;
        require_nonzero_full_parser_section(summary.semantic_node_bytes, "semantic PG nodes")?;
        require_nonzero_full_parser_section(summary.semantic_edge_bytes, "semantic PG edges")?;
        require_nonzero_full_parser_section(summary.sema_scope_bytes, "sema scope")?;
    }
    Ok(summary)
}

fn parser_bench_source_text(source: &SourceFile) -> Result<&str, String> {
    std::str::from_utf8(&source.bytes).map_err(|error| {
        format!(
            "Fix: C parser benchmark source `{}` is not valid UTF-8 at byte {}: {error}. The benchmark never repairs source bytes with lossy replacement because that would measure a different translation unit.",
            source.path.display(),
            error.valid_up_to()
        )
    })
}

fn parse_full_gpu_source_file(
    source: &SourceFile,
    backend: &CParserBenchBackend,
) -> Result<VyreBenchSummary, String> {
    let raw = String::from_utf8_lossy(&source.bytes);
    let options = backend.compile_options_for(source.path.clone());
    let resident = prepare_resident_translation_unit_source_gpu(&source.path, &raw, &options)
        .map_err(|error| {
            format!(
                "Fix: resident GPU preprocessing failed for `{}`: {error}",
                source.path.display()
            )
        })?;
    let summary = summary_from_c_parse(parse_source(&resident).map_err(|error| {
        format!(
            "Fix: full GPU parser pipeline failed for `{}`: {error}",
            source.path.display()
        )
    })?);
    if !resident.is_empty() {
        require_nonzero_full_parser_section(summary.ast_bytes, "AST")?;
        require_nonzero_full_parser_section(summary.vast_bytes, "VAST")?;
        require_nonzero_full_parser_section(summary.abi_layout_bytes, "ABI layout")?;
        require_nonzero_full_parser_section(summary.expression_shape_bytes, "expression shape")?;
        require_nonzero_full_parser_section(summary.program_graph_bytes, "program graph")?;
        require_nonzero_full_parser_section(summary.semantic_node_bytes, "semantic PG nodes")?;
        require_nonzero_full_parser_section(summary.semantic_edge_bytes, "semantic PG edges")?;
        require_nonzero_full_parser_section(summary.sema_scope_bytes, "sema scope")?;
    }
    Ok(summary)
}

fn summary_from_c_parse(summary: CParseSummary) -> VyreBenchSummary {
    VyreBenchSummary {
        token_count: summary.token_count,
        ast_bytes: summary.ast_bytes,
        vast_bytes: summary.vast_bytes,
        abi_layout_bytes: summary.abi_layout_bytes,
        expression_shape_bytes: summary.expression_shape_bytes,
        program_graph_bytes: summary.program_graph_bytes,
        semantic_node_bytes: summary.semantic_node_bytes,
        semantic_edge_bytes: summary.semantic_edge_bytes,
        sema_scope_bytes: summary.sema_scope_bytes,
        function_record_bytes: summary.function_record_bytes,
        call_record_bytes: summary.call_record_bytes,
    }
}

fn summary_from_syntax_parse(summary: SyntaxParseSummary) -> VyreBenchSummary {
    VyreBenchSummary {
        token_count: summary.token_count,
        ast_bytes: summary.ast_bytes,
        vast_bytes: 0,
        abi_layout_bytes: 0,
        expression_shape_bytes: 0,
        program_graph_bytes: 0,
        semantic_node_bytes: 0,
        semantic_edge_bytes: 0,
        sema_scope_bytes: 0,
        function_record_bytes: 0,
        call_record_bytes: 0,
    }
}

fn summary_from_full_object(object: &[u8], source_bytes: u64) -> Result<VyreBenchSummary, String> {
    let sections = inspect_vyrecob2_sections(object)?;
    let token_count = sections.lex_token_count.ok_or_else(|| {
        "Fix: full parser benchmark object is missing token-count evidence in the Lex section."
            .to_string()
    })?;
    let summary = VyreBenchSummary {
        token_count,
        ast_bytes: sections.ast_bytes,
        vast_bytes: sections.vast_bytes,
        abi_layout_bytes: sections.abi_layout_bytes,
        expression_shape_bytes: sections.expression_shape_bytes,
        program_graph_bytes: sections.program_graph_bytes,
        semantic_node_bytes: sections.semantic_node_bytes,
        semantic_edge_bytes: sections.semantic_edge_bytes,
        sema_scope_bytes: sections.sema_scope_bytes,
        function_record_bytes: sections.function_record_bytes,
        call_record_bytes: sections.call_record_bytes,
    };
    if source_bytes > 0 {
        require_nonzero_full_parser_section(summary.ast_bytes, "AST")?;
        require_nonzero_full_parser_section(summary.vast_bytes, "VAST")?;
        require_nonzero_full_parser_section(summary.abi_layout_bytes, "ABI layout")?;
        require_nonzero_full_parser_section(summary.expression_shape_bytes, "expression shape")?;
        require_nonzero_full_parser_section(summary.program_graph_bytes, "program graph")?;
        require_nonzero_full_parser_section(summary.semantic_node_bytes, "semantic PG nodes")?;
        require_nonzero_full_parser_section(summary.semantic_edge_bytes, "semantic PG edges")?;
        require_nonzero_full_parser_section(summary.sema_scope_bytes, "sema scope")?;
    }
    Ok(summary)
}

fn require_nonzero_full_parser_section(bytes: u64, section: &str) -> Result<(), String> {
    if bytes == 0 {
        return Err(format!(
            "Fix: full parser benchmark produced zero {section} bytes; release parser mode must prove AST plus semantic-analysis sections, not parser-only evidence."
        ));
    }
    Ok(())
}

#[derive(Default)]
struct FullObjectSections {
    lex_token_count: Option<u32>,
    ast_bytes: u64,
    vast_bytes: u64,
    abi_layout_bytes: u64,
    expression_shape_bytes: u64,
    program_graph_bytes: u64,
    semantic_node_bytes: u64,
    semantic_edge_bytes: u64,
    sema_scope_bytes: u64,
    function_record_bytes: u64,
    call_record_bytes: u64,
}

fn inspect_vyrecob2_sections(object: &[u8]) -> Result<FullObjectSections, String> {
    let start = find_magic(object, VYRECOB2_MAGIC)
        .ok_or_else(|| "Fix: full parser object does not embed a VYRECOB2 payload.".to_string())?;
    let mut cursor = start + VYRECOB2_MAGIC.len();
    let _version = read_u32(object, &mut cursor)
        .ok_or_else(|| "Fix: truncated VYRECOB2 version in full parser object.".to_string())?;
    let section_count = read_u32(object, &mut cursor).ok_or_else(|| {
        "Fix: truncated VYRECOB2 section count in full parser object.".to_string()
    })?;
    let mut sections = FullObjectSections::default();
    for _ in 0..section_count {
        let tag = read_u32(object, &mut cursor).ok_or_else(|| {
            "Fix: truncated VYRECOB2 section tag in full parser object.".to_string()
        })?;
        let len = read_u32(object, &mut cursor).ok_or_else(|| {
            "Fix: truncated VYRECOB2 section length in full parser object.".to_string()
        })? as usize;
        let end = cursor.checked_add(len).ok_or_else(|| {
            "Fix: VYRECOB2 section length overflow in full parser object.".to_string()
        })?;
        let payload = object.get(cursor..end).ok_or_else(|| {
            format!(
                "Fix: VYRECOB2 section tag {tag} length {len} exceeds object length {}.",
                object.len()
            )
        })?;
        match tag {
            tag if tag == SectionTag::Lex as u32 => {
                sections.lex_token_count = parse_lex_section_token_count(payload);
            }
            tag if tag == SectionTag::Ast as u32 => sections.ast_bytes = len as u64,
            tag if tag == SectionTag::Vast as u32 => sections.vast_bytes = len as u64,
            tag if tag == SectionTag::AbiLayout as u32 => sections.abi_layout_bytes = len as u64,
            tag if tag == SectionTag::ExpressionShape as u32 => {
                sections.expression_shape_bytes = len as u64;
            }
            tag if tag == SectionTag::ProgramGraph as u32 => {
                sections.program_graph_bytes = len as u64;
            }
            tag if tag == SectionTag::SemanticProgramGraphNodes as u32 => {
                sections.semantic_node_bytes = len as u64;
            }
            tag if tag == SectionTag::SemanticProgramGraphEdges as u32 => {
                sections.semantic_edge_bytes = len as u64;
            }
            tag if tag == SectionTag::SemaScope as u32 => sections.sema_scope_bytes = len as u64,
            tag if tag == SectionTag::Functions as u32 => {
                sections.function_record_bytes = len as u64;
            }
            tag if tag == SectionTag::Calls as u32 => sections.call_record_bytes = len as u64,
            _ => {}
        }
        cursor = end;
    }
    Ok(sections)
}

fn parse_lex_section_token_count(payload: &[u8]) -> Option<u32> {
    let mut cursor = 8usize;
    let _version = read_u32(payload, &mut cursor)?;
    let path_len = read_u32(payload, &mut cursor)? as usize;
    cursor = cursor.checked_add(path_len)?;
    while cursor % 8 != 0 {
        cursor = cursor.checked_add(1)?;
    }
    read_u32(payload, &mut cursor)
}

fn find_magic(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    let raw: [u8; 4] = bytes.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u32::from_le_bytes(raw))
}

fn temp_parse_object_path(source: &Path) -> PathBuf {
    let pid = std::process::id();
    let sequence = TEMP_PARSE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("tu");
    std::env::temp_dir().join(format!(
        "vyre-c-parser-bench-{stem}-{pid}-{nanos}-{sequence}.o"
    ))
}

fn build_batch_sources(sources: &[SourceFile], max_batch_bytes: usize) -> Vec<BenchBatch> {
    let max_batch_bytes = max_batch_bytes.max(1);
    let mut remaining_bytes = sources.iter().fold(0usize, |acc, source| {
        acc.saturating_add(source.bytes.len().saturating_add(3))
    });
    let mut batches = Vec::new();
    let mut batch = Vec::with_capacity(remaining_bytes.min(max_batch_bytes));
    let mut batch_start = 0usize;
    for (idx, source) in sources.iter().enumerate() {
        let needed = source.bytes.len().saturating_add(3);
        if !batch.is_empty() && batch.len().saturating_add(needed) > max_batch_bytes {
            remaining_bytes = remaining_bytes.saturating_sub(batch.len());
            batches.push(BenchBatch {
                bytes: batch,
                file_start: batch_start,
                file_end: idx,
            });
            batch_start = idx;
            batch = Vec::with_capacity(remaining_bytes.min(max_batch_bytes).max(needed));
        }
        batch.extend_from_slice(&source.bytes);
        batch.extend_from_slice(b"\n;\n");
    }
    if !batch.is_empty() {
        batches.push(BenchBatch {
            bytes: batch,
            file_start: batch_start,
            file_end: sources.len(),
        });
    }
    batches
}

fn run_per_file_benchmarks(
    mode: BenchMode,
    backend: &CParserBenchBackend,
    sources: &[SourceFile],
    parser: &mut tree_sitter::Parser,
    failures: &mut Vec<FileFailure>,
    run_vyre_per_file: bool,
) -> Result<Vec<FileBench>, String> {
    let mut benches = Vec::with_capacity(sources.len());
    for source in sources {
        let (vyre, vyre_wall_ns) = if run_vyre_per_file {
            let vyre_start = Instant::now();
            let vyre = match run_vyre_file(mode, backend, source) {
                Ok(summary) => summary,
                Err(error) => {
                    failures.push(FileFailure {
                        path: source.path.display().to_string(),
                        source_bytes: source.bytes.len() as u64,
                        stage: format!("vyre-per-file-{}", mode.as_str()),
                        error,
                    });
                    continue;
                }
            };
            (Some(vyre), Some(vyre_start.elapsed().as_nanos()))
        } else {
            (None, None)
        };

        let ts_start = Instant::now();
        let tree = parser.parse(&source.bytes, None).ok_or_else(|| {
            format!(
                "Fix: tree-sitter returned no parse tree for `{}`.",
                source.path.display()
            )
        })?;
        let tree_sitter_wall_ns = ts_start.elapsed().as_nanos();

        benches.push(FileBench {
            path: source.path.display().to_string(),
            source_bytes: source.bytes.len() as u64,
            vyre_tokens: vyre.as_ref().map(|summary| summary.token_count),
            vyre_ast_bytes: vyre.as_ref().map(|summary| summary.ast_bytes),
            vyre_vast_bytes: vyre.as_ref().map(|summary| summary.vast_bytes),
            vyre_abi_layout_bytes: vyre.as_ref().map(|summary| summary.abi_layout_bytes),
            vyre_expression_shape_bytes: vyre
                .as_ref()
                .map(|summary| summary.expression_shape_bytes),
            vyre_program_graph_bytes: vyre.as_ref().map(|summary| summary.program_graph_bytes),
            vyre_semantic_node_bytes: vyre.as_ref().map(|summary| summary.semantic_node_bytes),
            vyre_semantic_edge_bytes: vyre.as_ref().map(|summary| summary.semantic_edge_bytes),
            vyre_sema_scope_bytes: vyre.as_ref().map(|summary| summary.sema_scope_bytes),
            vyre_function_record_bytes: vyre.as_ref().map(|summary| summary.function_record_bytes),
            vyre_call_record_bytes: vyre.as_ref().map(|summary| summary.call_record_bytes),
            vyre_function_records: vyre
                .as_ref()
                .map(|summary| summary.function_record_bytes / 12),
            vyre_call_records: vyre.as_ref().map(|summary| summary.call_record_bytes / 16),
            vyre_wall_ns,
            tree_sitter_wall_ns,
            tree_sitter_has_error: tree.root_node().has_error(),
        });
    }
    Ok(benches)
}

fn collect_c_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| {
            format!(
                "Fix: failed to walk parser benchmark corpus `{}`: {error}",
                root.display()
            )
        })?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "c") {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut corpus = None;
    let mut output = None;
    let mut limit = None;
    let mut per_file = false;
    let mut warmups = 1usize;
    let mut max_batch_bytes = 512usize * 1024 * 1024;
    let mut mode = BenchMode::Syntax;
    let mut order = BenchOrder::Both;
    let mut backend = "preferred".to_string();
    let mut include_dirs = Vec::new();
    let mut macros = Vec::new();
    let mut require_release_evidence = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--corpus" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("Fix: --corpus requires a directory.".to_string());
                };
                corpus = Some(PathBuf::from(path));
            }
            "--output" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("Fix: --output requires a file path.".to_string());
                };
                output = Some(PathBuf::from(path));
            }
            "--limit" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("Fix: --limit requires a positive integer.".to_string());
                };
                let parsed = value
                    .parse::<usize>()
                    .map_err(|error| format!("Fix: invalid --limit `{value}`: {error}"))?;
                if parsed == 0 {
                    return Err("Fix: --limit must be greater than zero.".to_string());
                }
                limit = Some(parsed);
            }
            "--per-file" => {
                per_file = true;
            }
            "--require-release-evidence" => {
                require_release_evidence = true;
            }
            "--mode" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("Fix: --mode requires `syntax` or `parser`.".to_string());
                };
                mode = match value.as_str() {
                    "syntax" => BenchMode::Syntax,
                    "parser" => BenchMode::Parser,
                    _ => {
                        return Err(format!(
                            "Fix: invalid --mode `{value}`. Expected `syntax` or `parser`."
                        ))
                    }
                };
            }
            "--order" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err(
                        "Fix: --order requires `vyre-first`, `tree-sitter-first`, or `both`."
                            .to_string(),
                    );
                };
                order = match value.as_str() {
                    "vyre-first" => BenchOrder::VyreFirst,
                    "tree-sitter-first" => BenchOrder::TreeSitterFirst,
                    "both" => BenchOrder::Both,
                    _ => {
                        return Err(format!(
                            "Fix: invalid --order `{value}`. Expected `vyre-first`, `tree-sitter-first`, or `both`."
                        ))
                    }
                };
            }
            "--backend" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err(
                        "Fix: --backend requires `preferred`, `cuda`, or `wgpu`.".to_string()
                    );
                };
                backend = match value.as_str() {
                    "preferred" | "cuda" | "wgpu" => value.clone(),
                    _ => {
                        return Err(format!(
                            "Fix: invalid --backend `{value}`. Expected `preferred`, `cuda`, or `wgpu`."
                        ))
                    }
                };
            }
            "-I" | "--include" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    return Err("Fix: include option requires a directory.".to_string());
                };
                include_dirs.push(PathBuf::from(path));
            }
            "-D" | "--define" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("Fix: define option requires NAME or NAME=VALUE.".to_string());
                };
                macros.push(parse_define(value));
            }
            "--warmups" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("Fix: --warmups requires a non-negative integer.".to_string());
                };
                warmups = value
                    .parse::<usize>()
                    .map_err(|error| format!("Fix: invalid --warmups `{value}`: {error}"))?;
            }
            "--max-batch-bytes" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("Fix: --max-batch-bytes requires a positive integer.".to_string());
                };
                max_batch_bytes = value.parse::<usize>().map_err(|error| {
                    format!("Fix: invalid --max-batch-bytes `{value}`: {error}")
                })?;
                if max_batch_bytes == 0 {
                    return Err("Fix: --max-batch-bytes must be greater than zero.".to_string());
                }
            }
            "--help" | "-h" => {
                return Err(
                    "USAGE:\n  cargo_full run --bin xtask -- c-parser-bench --corpus DIR [--output PATH] [--limit N] [--warmups N] [--max-batch-bytes N] [--per-file] [--require-release-evidence] [--mode syntax|parser] [--order vyre-first|tree-sitter-first|both] [--backend preferred|cuda|wgpu]\n\n\
                     Batches every .c file into one corpus parse through vyre and tree-sitter-c, then writes cold/hot/order-aware speed evidence JSON. Use --mode syntax for raw-byte syntax throughput or --mode parser for the full GPU object parser/sema pipeline. Parser mode accepts repeated -I/--include and -D/--define options."
                        .to_string(),
                );
            }
            other => return Err(format!("Fix: unknown c-parser-bench option `{other}`.")),
        }
        i += 1;
    }
    let corpus = corpus.unwrap_or_else(default_corpus);
    let output = output
        .unwrap_or_else(|| PathBuf::from("release/evidence/parser/c-parser-subsystem-bench.json"));
    if require_release_evidence && limit.is_some() {
        return Err(
            "Fix: --require-release-evidence cannot be combined with --limit; release parser speed evidence must cover the full Linux corpus."
                .to_string(),
        );
    }
    if require_release_evidence && !is_full_linux_corpus(&corpus) {
        return Err(format!(
            "Fix: --require-release-evidence requires --corpus to point at the full Linux corpus root, not `{}`.",
            corpus.display()
        ));
    }
    if require_release_evidence && backend != "cuda" {
        return Err(format!(
            "Fix: --require-release-evidence requires explicit --backend cuda, not `{backend}`."
        ));
    }
    if require_release_evidence && mode != BenchMode::Parser {
        return Err(
            "Fix: --require-release-evidence requires --mode parser; syntax-only throughput is not parser release evidence."
                .to_string(),
        );
    }
    if require_release_evidence && order != BenchOrder::Both {
        return Err(
            "Fix: --require-release-evidence requires --order both so cold/hot/order-flipped measurements are present."
                .to_string(),
        );
    }
    if require_release_evidence && max_batch_bytes < 512usize * 1024 * 1024 {
        return Err(format!(
            "Fix: --require-release-evidence requires --max-batch-bytes >= 536870912, not {max_batch_bytes}."
        ));
    }
    if require_release_evidence && !per_file {
        return Err(
            "Fix: --require-release-evidence requires --per-file so tree-sitter comparator cleanliness is proven for every Linux source file."
                .to_string(),
        );
    }
    Ok(Config {
        corpus,
        output,
        limit,
        per_file,
        warmups,
        max_batch_bytes,
        mode,
        order,
        backend,
        include_dirs,
        macros,
        require_release_evidence,
    })
}

fn parse_define(value: &str) -> (String, Option<String>) {
    match value.split_once('=') {
        Some((name, body)) => (name.to_string(), Some(body.to_string())),
        None => (value.to_string(), None),
    }
}

fn default_corpus() -> PathBuf {
    PathBuf::from("/media/mukund-thiru/SanthData/Santh/corpus/repos/linux/lib")
}

#[cfg(test)]
mod tests {
    use super::{
        is_release_evidence_valid, parse_args, parser_bench_source_text, repro_command, BenchOrder,
        SourceFile,
    };
    use std::path::PathBuf;

    #[test]
    fn release_evidence_requires_comparator_trees_and_10x_all_orders() {
        let valid = |full_linux: bool,
                     file_count: usize,
                     failure_count: usize,
                     tree_sitter_has_error: bool,
                     tree_sitter_error_batches: usize,
                     tree_sitter_parse_failures: usize,
                     per_file_tree_sitter_error_count: usize,
                     per_file_enabled: bool,
                     per_file_entry_count: usize,
                     vyre_tokens: u32,
                     artifact_bytes: u64,
                     cold_speedup: u128,
                     hot_speedup: u128,
                     reverse_speedup: u128| {
            is_release_evidence_valid(
                full_linux,
                file_count,
                failure_count,
                tree_sitter_has_error,
                tree_sitter_error_batches,
                tree_sitter_parse_failures,
                per_file_tree_sitter_error_count,
                per_file_enabled,
                per_file_entry_count,
                vyre_tokens,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes,
                artifact_bytes / 12,
                cold_speedup,
                hot_speedup,
                reverse_speedup,
                BenchOrder::Both,
            )
        };
        assert!(valid(
            true, 30_000, 0, false, 0, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, true, 0, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 1, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 1, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 1, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 0, true, 30_000, 0, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 0, true, 30_000, 1, 0, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 10_999
        ));
        assert!(!valid(
            true, 29_999, 0, false, 0, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            false, 30_000, 0, false, 0, 0, 0, true, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 0, false, 30_000, 1, 192, 11_000, 11_000, 11_000
        ));
        assert!(!valid(
            true, 30_000, 0, false, 0, 0, 0, true, 29_999, 1, 192, 11_000, 11_000, 11_000
        ));
    }

    #[test]
    fn require_release_evidence_flag_is_parsed() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--per-file".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let config = parse_args(&args).expect("Fix: benchmark args should parse");
        assert!(config.require_release_evidence);
        let repro = repro_command(&config);
        assert!(repro.contains("cargo_full run --release --bin xtask -- c-parser-bench"));
        assert!(repro.contains("--require-release-evidence"));
    }

    #[test]
    fn parser_benchmark_rejects_lossy_source_text_repair() {
        let source = SourceFile {
            path: PathBuf::from("bad.c"),
            bytes: vec![b'i', b'n', b't', b' ', 0xff],
        };
        let error = parser_bench_source_text(&source)
            .expect_err("invalid UTF-8 must not be repaired with replacement chars");
        assert!(error.contains("never repairs source bytes with lossy replacement"));
    }

    #[test]
    fn release_evidence_rejects_limited_corpus() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--limit".to_string(),
            "1".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err = parse_args(&args).expect_err("limited release evidence must be rejected");
        assert!(err.contains("cannot be combined with --limit"));
    }

    #[test]
    fn release_evidence_requires_per_file_comparator_cleanliness() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err = parse_args(&args).expect_err("release evidence must require --per-file");
        assert!(err.contains("requires --per-file"));
    }

    #[test]
    fn release_evidence_rejects_subsystem_corpus() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux/lib".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err = parse_args(&args).expect_err("subsystem release evidence must be rejected");
        assert!(err.contains("full Linux corpus root"));
    }

    #[test]
    fn release_evidence_requires_explicit_cuda_backend() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err =
            parse_args(&args).expect_err("implicit backend release evidence must be rejected");
        assert!(err.contains("explicit --backend cuda"));
    }

    #[test]
    fn release_evidence_requires_parser_mode_and_order_both() {
        let syntax_args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "syntax".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err = parse_args(&syntax_args).expect_err("syntax release evidence must be rejected");
        assert!(err.contains("--mode parser"));

        let order_args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "vyre-first".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err =
            parse_args(&order_args).expect_err("single-order release evidence must be rejected");
        assert!(err.contains("--order both"));
    }

    #[test]
    fn release_evidence_requires_large_cuda_batch_cap() {
        let args = vec![
            "xtask".to_string(),
            "c-parser-bench".to_string(),
            "--corpus".to_string(),
            "/tmp/linux".to_string(),
            "--backend".to_string(),
            "cuda".to_string(),
            "--mode".to_string(),
            "parser".to_string(),
            "--order".to_string(),
            "both".to_string(),
            "--max-batch-bytes".to_string(),
            "67108864".to_string(),
            "--require-release-evidence".to_string(),
        ];
        let err = parse_args(&args).expect_err("small-batch release evidence must be rejected");
        assert!(err.contains("--max-batch-bytes >= 536870912"));
    }
}

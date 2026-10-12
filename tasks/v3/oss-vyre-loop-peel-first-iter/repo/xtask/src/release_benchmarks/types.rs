use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) const REQUIRED_CPU_SOTA_100X_CASES: &[&str] = &[
    "release.condition_eval.1m",
    "release.string_bitmap_scatter.1m",
    "release.offset_count_aggregation.1m",
    "release.entropy_window.1m",
    "release.quantified_condition_loops.1m",
    "release.alias_reaching_def.1m",
    "release.ifds_witness.1m",
    "release.c_ast_traversal.1m",
    "release.megakernel_queue.1m",
    "release.egraph_saturation.1m",
    "sparse.compaction.count.1m",
];
pub(super) const MIN_CPU_SOTA_100X_RELEASE_CASES: usize = 10;
pub(super) const MAX_RELEASE_BENCHMARK_TEXT_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const MIN_CUDA_RELEASE_MEMORY_MIB: u64 = 16 * 1024;
pub(super) const MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MAJOR: u64 = 8;
pub(super) const MIN_CUDA_RELEASE_COMPUTE_CAPABILITY_MINOR: u64 = 0;

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseWorkloadMatrix {
    pub(super) families: Vec<ReleaseWorkloadFamily>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseWorkloadFamily {
    pub(super) id: String,
    pub(super) required: bool,
    pub(super) matched_cases: Vec<String>,
    pub(super) evidence_artifact: String,
    #[serde(default)]
    pub(super) max_cpu_sota_min_speedup_x: Option<f64>,
    #[serde(default)]
    pub(super) cpu_sota_100x_cases: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct BackendSuiteEvidence {
    pub(super) schema_version: u32,
    pub(super) backend: String,
    pub(super) schema_digest_chain: Value,
    pub(super) hardware_digest: String,
    pub(super) hardware_digest_fields: Vec<HardwareDigestField>,
    pub(super) hardware_unavailable_reasons: Vec<HardwareUnavailableReason>,
    pub(super) family_count: usize,
    pub(super) artifacts: Vec<String>,
    pub(super) artifact_statuses: Vec<BackendSuiteArtifact>,
    pub(super) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct HardwareDigestField {
    pub(super) field: String,
    pub(super) value: String,
    pub(super) source: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct HardwareUnavailableReason {
    pub(super) field: String,
    pub(super) reason: String,
    pub(super) fix: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BackendSuiteArtifact {
    pub(super) path: String,
    pub(super) family_id: String,
    pub(super) requested_case_id: String,
    pub(super) exists: bool,
    pub(super) bytes: u64,
    pub(super) read_error: Option<String>,
    pub(super) source_fingerprint: Option<String>,
    pub(super) source_tree_fingerprint: Option<String>,
    pub(super) selected_backend: Option<String>,
    pub(super) host_cpu_model: Option<String>,
    pub(super) gpu_model: Option<String>,
    pub(super) gpu_memory_total_mib: Option<u64>,
    pub(super) gpu_compute_capability_major: Option<u64>,
    pub(super) gpu_compute_capability_minor: Option<u64>,
    pub(super) nvidia_driver_version: Option<String>,
    pub(super) nvidia_cuda_version: Option<String>,
    pub(super) min_cuda_ptx_source_cache_entries: Option<u64>,
    pub(super) min_cuda_ptx_source_cache_hits: Option<u64>,
    pub(super) min_cuda_ptx_source_cache_misses: Option<u64>,
    pub(super) min_kernel_launches: Option<u64>,
    pub(super) fused_execution_dag_contract: Option<String>,
    pub(super) fused_execution_dag_node_count: Option<usize>,
    pub(super) fused_execution_dag_memory_edge_count: Option<usize>,
    pub(super) fused_execution_dag_host_sync_points: Option<u64>,
    pub(super) case_count: usize,
    pub(super) failed_count: Option<u64>,
    pub(super) nonmatching_case_backend_count: usize,
    pub(super) min_wall_samples: Option<u64>,
    pub(super) min_wall_p50: Option<u64>,
    pub(super) min_wall_p95: Option<u64>,
    pub(super) min_wall_p99: Option<u64>,
    pub(super) min_baseline_wall_samples: Option<u64>,
    pub(super) min_baseline_wall_p50: Option<u64>,
    pub(super) min_baseline_wall_p95: Option<u64>,
    pub(super) min_baseline_wall_p99: Option<u64>,
    pub(super) cpu_sota_100x_required: bool,
    pub(super) cpu_sota_100x_contract_cases: usize,
    pub(super) cpu_sota_100x_passing_cases: usize,
    pub(super) blockers: Vec<String>,
}

#[derive(Debug)]
pub(super) struct BackendSuiteArtifactInput {
    pub(super) path: String,
    pub(super) family_id: String,
    pub(super) requested_case_id: String,
    pub(super) cpu_sota_100x_required: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct ReleaseAxesEvidence {
    pub(super) schema_version: u32,
    pub(super) warm_us_per_file: Option<f64>,
    pub(super) cold_pipeline_build_ms: Option<f64>,
    pub(super) gbs_scan_throughput: Option<f64>,
    pub(super) ulp_drift_max: Option<u32>,
    pub(super) max_vram_mib: Option<u64>,
    pub(super) source_artifacts: Vec<String>,
    pub(super) blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct OptimizationBenchmarkManifest {
    pub(super) schema_version: u32,
    pub(super) backend: String,
    pub(super) selected_backend: String,
    pub(super) source_fingerprint: String,
    pub(super) source_tree_fingerprint: String,
    pub(super) environment: Value,
    pub(super) summary: OptimizationBenchmarkManifestSummary,
    pub(super) required_case_count: usize,
    pub(super) required_pass_families: Vec<&'static str>,
    pub(super) covered_pass_families: Vec<&'static str>,
    pub(super) uncovered_pass_families: Vec<&'static str>,
    pub(super) cases: Vec<OptimizationBenchmarkEvidence>,
    pub(super) blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct OptimizationBenchmarkManifestSummary {
    pub(super) total_cases: usize,
    pub(super) passed: usize,
    pub(super) failed: usize,
    pub(super) total_time_ns: u64,
    pub(super) cache_hit_rate: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(super) struct OptimizationBenchmarkEvidence {
    pub(super) id: &'static str,
    pub(super) case_id: &'static str,
    pub(super) artifact: &'static str,
    pub(super) backend_id: String,
    pub(super) status: String,
    pub(super) covered_pass_families: Vec<&'static str>,
    pub(super) required_custom_metrics: Vec<&'static str>,
    pub(super) required_positive_metrics: Vec<&'static str>,
    pub(super) exists: bool,
    pub(super) read_error: Option<String>,
    pub(super) case_count: usize,
    pub(super) min_wall_samples: Option<u64>,
    pub(super) min_wall_p50: Option<u64>,
    pub(super) min_wall_p95: Option<u64>,
    pub(super) min_wall_p99: Option<u64>,
    pub(super) min_baseline_wall_samples: Option<u64>,
    pub(super) min_baseline_wall_p50: Option<u64>,
    pub(super) min_baseline_wall_p95: Option<u64>,
    pub(super) min_baseline_wall_p99: Option<u64>,
    pub(super) min_wall_speedup_x1000: Option<u64>,
    pub(super) missing_custom_metrics: Vec<String>,
    pub(super) non_positive_required_metrics: Vec<String>,
    pub(super) non_winning_cases: Vec<String>,
    pub(super) blockers: Vec<String>,
}

#[derive(Debug)]
pub(super) struct OptimizationArtifactInspection {
    pub(super) exists: bool,
    pub(super) read_error: Option<String>,
    pub(super) selected_backend: Option<String>,
    pub(super) source_fingerprint: Option<String>,
    pub(super) source_tree_fingerprint: Option<String>,
    pub(super) environment: Option<Value>,
    pub(super) summary_total_time_ns: Option<u64>,
    pub(super) summary_cache_hit_rate: Option<f64>,
    pub(super) case_count: usize,
    pub(super) min_wall_samples: Option<u64>,
    pub(super) min_wall_p50: Option<u64>,
    pub(super) min_wall_p95: Option<u64>,
    pub(super) min_wall_p99: Option<u64>,
    pub(super) min_baseline_wall_samples: Option<u64>,
    pub(super) min_baseline_wall_p50: Option<u64>,
    pub(super) min_baseline_wall_p95: Option<u64>,
    pub(super) min_baseline_wall_p99: Option<u64>,
    pub(super) min_wall_speedup_x1000: Option<u64>,
    pub(super) missing_custom_metrics: Vec<String>,
    pub(super) non_positive_required_metrics: Vec<String>,
    pub(super) non_winning_cases: Vec<String>,
    pub(super) blockers: Vec<String>,
}

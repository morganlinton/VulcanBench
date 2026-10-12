use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::api::case::{Correctness, PerformanceContract, PerformanceEvaluation};
use crate::api::metric::MetricStats;
use crate::probes::environment::EnvironmentData;

pub const REQUIRED_BENCHMARK_CASE_FIELDS: &[&str] =
    &["backend_id", "device_signature", "held_out_corpus_id"];
pub const REQUIRED_BENCHMARK_METRIC_FIELDS: &[&str] =
    &["cpu_digest", "gpu_digest", "active_time_ns", "transfer_bytes"];
pub const REQUIRED_SCAN_BENCHMARK_METRIC_FIELDS: &[&str] = &[
    "scan_compile_time_ns",
    "scan_database_bytes",
    "scan_scratch_bytes",
    "scan_cold_throughput_bytes_per_s",
    "scan_warm_throughput_bytes_per_s",
    "scan_streaming_setup_ns",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSchema {
    pub schema: String,
    pub run_id: String,
    pub suite: String,
    #[serde(default)]
    pub selected_backend: Option<String>,
    #[serde(default)]
    pub backend_profile: Option<ReportBackendProfile>,
    pub git: BTreeMap<String, String>,
    #[serde(default)]
    pub source_fingerprint: String,
    #[serde(default)]
    pub source_tree_fingerprint: String,
    pub environment: EnvironmentData,
    pub features: Vec<String>,
    pub cases: Vec<CaseReport>,
    pub summary: ReportSummary,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportBackendProfile {
    pub backend: String,
    pub timing_quality: String,
    pub supports_device_timestamps: bool,
    pub supports_hardware_counters: bool,
    pub supports_subgroup_ops: bool,
    pub supports_indirect_dispatch: bool,
    pub max_workgroup_size: [u32; 3],
    pub max_invocations_per_workgroup: u32,
    pub max_shared_memory_bytes: u32,
    pub max_storage_buffer_binding_size: u64,
    pub subgroup_size: u32,
    pub compute_units: u32,
    pub mem_bw_gbps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanBenchmarkMetricEvidence {
    pub case_id: String,
    pub compile_time_ns: u64,
    pub database_bytes: u64,
    pub scratch_bytes: u64,
    pub cold_throughput_bytes_per_s: u64,
    pub warm_throughput_bytes_per_s: u64,
    pub streaming_setup_ns: u64,
}

impl ScanBenchmarkMetricEvidence {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.case_id.is_empty()
            && self.compile_time_ns != 0
            && self.database_bytes != 0
            && self.cold_throughput_bytes_per_s != 0
            && self.warm_throughput_bytes_per_s != 0
    }
}

pub fn validate_scan_benchmark_metric_evidence(
    evidence: &ScanBenchmarkMetricEvidence,
) -> Result<(), String> {
    if evidence.case_id.is_empty() {
        return Err("Fix: scan benchmark metric evidence case_id must be non-empty.".to_string());
    }
    if evidence.compile_time_ns == 0 {
        return Err(format!(
            "Fix: scan benchmark metric evidence `{}` is missing compile_time_ns.",
            evidence.case_id
        ));
    }
    if evidence.database_bytes == 0 {
        return Err(format!(
            "Fix: scan benchmark metric evidence `{}` is missing database_bytes.",
            evidence.case_id
        ));
    }
    if evidence.cold_throughput_bytes_per_s == 0 {
        return Err(format!(
            "Fix: scan benchmark metric evidence `{}` is missing cold throughput.",
            evidence.case_id
        ));
    }
    if evidence.warm_throughput_bytes_per_s == 0 {
        return Err(format!(
            "Fix: scan benchmark metric evidence `{}` is missing warm throughput.",
            evidence.case_id
        ));
    }
    Ok(())
}

impl ReportBackendProfile {
    #[must_use]
    pub fn from_device_profile(profile: vyre_driver::DeviceProfile) -> Self {
        Self {
            backend: profile.backend.to_string(),
            timing_quality: profile.timing_quality.as_str().to_string(),
            supports_device_timestamps: profile.supports_device_timestamps,
            supports_hardware_counters: profile.supports_hardware_counters,
            supports_subgroup_ops: profile.supports_subgroup_ops,
            supports_indirect_dispatch: profile.supports_indirect_dispatch,
            max_workgroup_size: profile.max_workgroup_size,
            max_invocations_per_workgroup: profile.max_invocations_per_workgroup,
            max_shared_memory_bytes: profile.max_shared_memory_bytes,
            max_storage_buffer_binding_size: profile.max_storage_buffer_binding_size,
            subgroup_size: profile.subgroup_size,
            compute_units: profile.compute_units,
            mem_bw_gbps: profile.mem_bw_gbps,
        }
    }

    #[must_use]
    pub fn has_valid_timing_quality(&self) -> bool {
        matches!(
            self.timing_quality.as_str(),
            "host_only" | "host_enqueue_wait" | "device_timestamps" | "hardware_counters"
        )
    }
}

#[must_use]
pub fn benchmark_device_signature(profile: vyre_driver::DeviceProfile) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-bench.device-profile.v1");
    update_signature_str(&mut hasher, "backend", profile.backend);
    update_signature_str(&mut hasher, "timing_quality", profile.timing_quality.as_str());
    update_signature_bool(
        &mut hasher,
        "supports_subgroup_ops",
        profile.supports_subgroup_ops,
    );
    update_signature_bool(
        &mut hasher,
        "supports_indirect_dispatch",
        profile.supports_indirect_dispatch,
    );
    update_signature_bool(
        &mut hasher,
        "supports_distributed_collectives",
        profile.supports_distributed_collectives,
    );
    update_signature_bool(
        &mut hasher,
        "supports_specialization_constants",
        profile.supports_specialization_constants,
    );
    update_signature_bool(&mut hasher, "supports_f16", profile.supports_f16);
    update_signature_bool(&mut hasher, "supports_bf16", profile.supports_bf16);
    update_signature_bool(
        &mut hasher,
        "supports_trap_propagation",
        profile.supports_trap_propagation,
    );
    update_signature_bool(
        &mut hasher,
        "supports_tensor_cores",
        profile.supports_tensor_cores,
    );
    update_signature_bool(&mut hasher, "has_mul_high", profile.has_mul_high);
    update_signature_bool(
        &mut hasher,
        "has_dual_issue_fp32_int32",
        profile.has_dual_issue_fp32_int32,
    );
    update_signature_bool(
        &mut hasher,
        "has_subgroup_shuffle",
        profile.has_subgroup_shuffle,
    );
    update_signature_bool(&mut hasher, "has_shared_memory", profile.has_shared_memory);
    update_signature_u32(&mut hasher, "max_native_int_width", profile.max_native_int_width);
    for (axis, value) in profile.max_workgroup_size.iter().enumerate() {
        update_signature_u32(&mut hasher, &format!("max_workgroup_size.{axis}"), *value);
    }
    update_signature_u32(
        &mut hasher,
        "max_invocations_per_workgroup",
        profile.max_invocations_per_workgroup,
    );
    update_signature_u32(
        &mut hasher,
        "max_shared_memory_bytes",
        profile.max_shared_memory_bytes,
    );
    update_signature_u64(
        &mut hasher,
        "max_storage_buffer_binding_size",
        profile.max_storage_buffer_binding_size,
    );
    update_signature_u32(&mut hasher, "subgroup_size", profile.subgroup_size);
    update_signature_u32(&mut hasher, "compute_units", profile.compute_units);
    update_signature_u32(&mut hasher, "regs_per_thread_max", profile.regs_per_thread_max);
    update_signature_u32(&mut hasher, "l1_cache_bytes", profile.l1_cache_bytes);
    update_signature_u32(&mut hasher, "l2_cache_bytes", profile.l2_cache_bytes);
    update_signature_u32(&mut hasher, "mem_bw_gbps", profile.mem_bw_gbps);
    update_signature_bool(
        &mut hasher,
        "supports_device_timestamps",
        profile.supports_device_timestamps,
    );
    update_signature_bool(
        &mut hasher,
        "supports_hardware_counters",
        profile.supports_hardware_counters,
    );
    update_signature_u32(&mut hasher, "ideal_unroll_depth", profile.ideal_unroll_depth);
    update_signature_u32(
        &mut hasher,
        "ideal_vector_pack_bits",
        profile.ideal_vector_pack_bits,
    );
    for (axis, value) in profile.ideal_workgroup_tile.iter().enumerate() {
        update_signature_u32(&mut hasher, &format!("ideal_workgroup_tile.{axis}"), *value);
    }
    update_signature_u32(
        &mut hasher,
        "shared_memory_bank_count",
        profile.shared_memory_bank_count,
    );
    update_signature_u32(
        &mut hasher,
        "shared_memory_bank_width_bytes",
        profile.shared_memory_bank_width_bytes,
    );
    format!("device-profile-v1:{}", hasher.finalize().to_hex())
}

#[must_use]
pub fn benchmark_held_out_corpus_id(workload_fingerprint: &str) -> String {
    format!("heldout:{workload_fingerprint}")
}

fn update_signature_str(hasher: &mut blake3::Hasher, name: &str, value: &str) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn update_signature_bool(hasher: &mut blake3::Hasher, name: &str, value: bool) {
    update_signature_str(hasher, name, if value { "1" } else { "0" });
}

fn update_signature_u32(hasher: &mut blake3::Hasher, name: &str, value: u32) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&value.to_le_bytes());
}

fn update_signature_u64(hasher: &mut blake3::Hasher, name: &str, value: u64) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&value.to_le_bytes());
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CaseReport {
    pub id: String,
    #[serde(default)]
    pub workload_fingerprint: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner_crate: String,
    #[serde(default)]
    pub workload_class: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub backend_id: Option<String>,
    #[serde(default)]
    pub device_signature: Option<String>,
    #[serde(default)]
    pub held_out_corpus_id: Option<String>,
    #[serde(default)]
    pub needs_gpu: bool,
    #[serde(default)]
    pub min_vram_bytes: Option<u64>,
    #[serde(default)]
    pub min_input_bytes: Option<u64>,
    #[serde(default)]
    pub required_features: Vec<String>,
    pub status: String,
    pub wall_ns: Option<f64>,
    pub correctness: Correctness,
    pub contract: Option<PerformanceContract>,
    pub performance: Option<PerformanceEvaluation>,
    pub metrics: BTreeMap<String, MetricStats>,
    #[serde(default)]
    pub optimization_passes_applied: Vec<String>,
    pub artifacts: Vec<String>,
}

pub const LOWER_FULL_REPORT_ARTIFACT_KIND: &str = "vyre.lower.full_report.json";

#[derive(Serialize)]
struct LowerFullReportArtifact<'a> {
    kind: &'static str,
    descriptor_id: &'a str,
    verify_input_status: &'static str,
    verify_output_status: &'static str,
    histogram: &'a vyre_lower::analyses::op_histogram::OpHistogram,
    rewrite_stats: &'a vyre_lower::rewrites::OptimizationStats,
    fix_text: &'a str,
    full_report: &'a vyre_lower::FullReport,
}

pub fn lower_full_report_artifact(
    report: &vyre_lower::FullReport,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&LowerFullReportArtifact {
        kind: LOWER_FULL_REPORT_ARTIFACT_KIND,
        descriptor_id: &report.descriptor_id,
        verify_input_status: report.verify_input_status(),
        verify_output_status: report.verify_output_status(),
        histogram: &report.histogram,
        rewrite_stats: &report.stats,
        fix_text: &report.fix_text,
        full_report: report,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_time_ns: u64,
    pub cache_hit_rate: Option<f64>,
}

impl CaseReport {
    pub fn passes_summary_evidence(&self) -> bool {
        self.status == "pass"
            && !matches!(self.correctness, Correctness::Invalid { .. })
            && !self
                .performance
                .as_ref()
                .is_some_and(|performance| !performance.contract_passed)
    }

    pub fn evidence_blockers(&self) -> Vec<String> {
        let mut blockers = Vec::new();
        if self.status != "pass" {
            blockers.push(format!("case `{}` status `{}`", self.id, self.status));
        }
        if let Correctness::Invalid { reason } = &self.correctness {
            blockers.push(format!("case `{}` correctness invalid: {reason}", self.id));
        }
        if let Some(performance) = &self.performance {
            if !performance.contract_passed {
                if performance.violations.is_empty() {
                    blockers.push(format!(
                        "case `{}` failed its performance contract without a violation reason",
                        self.id
                    ));
                } else {
                    for violation in &performance.violations {
                        blockers.push(format!(
                            "case `{}` performance contract failed: {violation}",
                            self.id
                        ));
                    }
                }
            }
        }
        blockers
    }

    pub fn validate_benchmark_evidence_schema(&self) -> Result<(), String> {
        if self.backend_id.as_deref().is_none_or(str::is_empty) {
            return Err(format!(
                "case `{}` is missing benchmark evidence field `backend_id`. Fix: regenerate the benchmark report with a current vyre-bench runner.",
                self.id
            ));
        }
        if self.device_signature.as_deref().is_none_or(str::is_empty) {
            return Err(format!(
                "case `{}` is missing benchmark evidence field `device_signature`. Fix: regenerate the benchmark report so the selected backend DeviceProfile is recorded.",
                self.id
            ));
        }
        if self.held_out_corpus_id.as_deref().is_none_or(str::is_empty) {
            return Err(format!(
                "case `{}` is missing benchmark evidence field `held_out_corpus_id`. Fix: regenerate the benchmark report so the workload corpus identity is recorded.",
                self.id
            ));
        }
        for metric in REQUIRED_BENCHMARK_METRIC_FIELDS {
            let stats = self.metrics.get(*metric).ok_or_else(|| {
                format!(
                    "case `{}` is missing required benchmark metric `{metric}`. Fix: regenerate the benchmark report with CPU digest, GPU digest, active time, and transfer-byte normalization enabled.",
                    self.id
                )
            })?;
            if stats.samples == 0 {
                return Err(format!(
                    "case `{}` required benchmark metric `{metric}` has zero samples. Fix: regenerate the benchmark report from measured evidence instead of hand-edited JSON.",
                    self.id
                ));
            }
        }
        if self
            .metrics
            .get("active_time_ns")
            .is_some_and(|stats| stats.max == 0)
        {
            return Err(format!(
                "case `{}` required benchmark metric `active_time_ns` is zero. Fix: collect dispatch, kernel, or wall timing for the case.",
                self.id
            ));
        }
        if self.is_scan_case() {
            for metric in REQUIRED_SCAN_BENCHMARK_METRIC_FIELDS {
                let stats = self.metrics.get(*metric).ok_or_else(|| {
                    format!(
                        "case `{}` is missing required scan benchmark metric `{metric}`. Fix: regenerate scan benchmark JSON with compile time, database bytes, scratch bytes, cold/warm throughput, and streaming setup metrics.",
                        self.id
                    )
                })?;
                if stats.samples == 0 {
                    return Err(format!(
                        "case `{}` scan benchmark metric `{metric}` has zero samples. Fix: regenerate scan benchmark JSON from measured compile and scan evidence.",
                        self.id
                    ));
                }
            }
        }
        Ok(())
    }

    fn is_scan_case(&self) -> bool {
        self.id.contains("scan") || self.tags.iter().any(|tag| tag == "scan")
    }
}

impl ReportSchema {
    pub fn evidence_summary_counts(&self) -> (usize, usize) {
        let passed = self
            .cases
            .iter()
            .filter(|case| case.passes_summary_evidence())
            .count();
        (passed, self.cases.len().saturating_sub(passed))
    }

    pub fn validate_summary_evidence(&self) -> Result<(), String> {
        if self.summary.total_cases != self.cases.len() {
            return Err(format!(
                "summary.total_cases={} does not match {} case report(s). Fix: regenerate the benchmark report from case evidence.",
                self.summary.total_cases,
                self.cases.len()
            ));
        }
        let (passed, failed) = self.evidence_summary_counts();
        if self.summary.passed != passed || self.summary.failed != failed {
            return Err(format!(
                "summary pass/fail ({}/{}) contradicts case evidence ({}/{}). Fix: regenerate the benchmark report from case status, correctness, and performance contracts.",
                self.summary.passed, self.summary.failed, passed, failed
            ));
        }
        Ok(())
    }

    pub fn validate_blocker_evidence(&self) -> Result<(), String> {
        let derived = self.derived_blockers();
        if self.blockers != derived {
            return Err(format!(
                "top-level blockers {:?} contradict case-derived blockers {:?}. Fix: regenerate the benchmark report from case status, correctness, and performance contracts.",
                self.blockers, derived
            ));
        }
        Ok(())
    }

    pub fn validate_backend_profile_evidence(
        &self,
        expected_backend: Option<&str>,
    ) -> Result<(), String> {
        let expected_backend = expected_backend.or(self.selected_backend.as_deref());
        if let Some(expected_backend) = expected_backend {
            let profile = self.backend_profile.as_ref().ok_or_else(|| {
                format!(
                    "report for backend `{expected_backend}` lacks backend_profile. Fix: regenerate the benchmark report with a current vyre-bench binary so backend profile and timing-quality evidence are recorded."
                )
            })?;
            if profile.backend != expected_backend {
                return Err(format!(
                    "backend_profile.backend `{}` contradicts expected backend `{expected_backend}`. Fix: regenerate the report from the selected backend instead of editing JSON by hand.",
                    profile.backend
                ));
            }
        }
        if let Some(selected_backend) = self.selected_backend.as_deref() {
            if let Some(profile) = self.backend_profile.as_ref() {
                if profile.backend != selected_backend {
                    return Err(format!(
                        "backend_profile.backend `{}` contradicts selected_backend `{selected_backend}`. Fix: regenerate the benchmark report from one backend acquisition path.",
                        profile.backend
                    ));
                }
            }
        }
        if let Some(profile) = self.backend_profile.as_ref() {
            if !profile.has_valid_timing_quality() {
                return Err(format!(
                    "backend_profile.timing_quality `{}` is not a stable timing-quality value. Fix: use DeviceTimingQuality::as_str() when generating reports.",
                    profile.timing_quality
                ));
            }
            if profile.max_workgroup_size.contains(&0) {
                return Err(format!(
                    "backend_profile.max_workgroup_size {:?} contains zero. Fix: report conservative nonzero dispatch limits for benchmark evidence.",
                    profile.max_workgroup_size
                ));
            }
            if profile.max_invocations_per_workgroup == 0 {
                return Err(
                    "backend_profile.max_invocations_per_workgroup is zero. Fix: report a conservative nonzero invocation limit for benchmark evidence."
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    pub fn validate_benchmark_case_evidence_schema(&self) -> Result<(), String> {
        for case in &self.cases {
            case.validate_benchmark_evidence_schema()?;
        }
        Ok(())
    }

    pub fn derived_blockers(&self) -> Vec<String> {
        self.cases
            .iter()
            .flat_map(CaseReport::evidence_blockers)
            .collect()
    }
}

pub fn generate_json_report(report: &ReportSchema) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case_report(
        status: &str,
        correctness: Correctness,
        performance: Option<PerformanceEvaluation>,
    ) -> CaseReport {
        CaseReport {
            id: "release.condition_eval.1m".to_string(),
            workload_fingerprint: "bench-case:release.condition_eval.1m".to_string(),
            name: "release condition eval".to_string(),
            owner_crate: "vyre-bench".to_string(),
            workload_class: "Release".to_string(),
            tags: Vec::new(),
            backend_id: Some("cuda".to_string()),
            device_signature: Some("device-profile-v1:test".to_string()),
            held_out_corpus_id: Some("heldout:bench-case:release.condition_eval.1m".to_string()),
            needs_gpu: true,
            min_vram_bytes: None,
            min_input_bytes: None,
            required_features: Vec::new(),
            status: status.to_string(),
            wall_ns: Some(1.0),
            correctness,
            contract: None,
            performance,
            metrics: benchmark_evidence_metrics(),
            optimization_passes_applied: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    fn benchmark_evidence_metrics() -> BTreeMap<String, MetricStats> {
        let mut metrics = BTreeMap::new();
        for (name, value) in [
            ("cpu_digest", 1),
            ("gpu_digest", 1),
            ("active_time_ns", 10),
            ("transfer_bytes", 64),
        ] {
            metrics.insert(name.to_string(), stats(value));
        }
        metrics
    }

    fn stats(value: u64) -> MetricStats {
        MetricStats {
            min: value,
            p50: value,
            p90: value,
            p95: value,
            p99: value,
            p999: value,
            p9999: value,
            max: value,
            mean: value as f64,
            stddev: 0.0,
            samples: 1,
            determinism_cv: None,
        }
    }

    fn performance(contract_passed: bool) -> PerformanceEvaluation {
        PerformanceEvaluation {
            speedup_x: Some(100.0),
            contract_passed,
            violations: if contract_passed {
                Vec::new()
            } else {
                vec!["speedup below release floor".to_string()]
            },
        }
    }

    #[test]
    fn lower_full_report_artifact_carries_lower_evidence_surface() {
        let desc = vyre_lower::KernelDescriptor {
            id: "bench_bad".to_string(),
            bindings: vyre_lower::BindingLayout { slots: vec![] },
            dispatch: vyre_lower::Dispatch::new(0, 1, 1),
            body: vyre_lower::KernelBody {
                ops: vec![vyre_lower::KernelOp {
                    kind: vyre_lower::KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![vyre_lower::LiteralValue::U32(7)],
            },
        };
        let lower_report = vyre_lower::full_report(&desc);
        let artifact =
            lower_full_report_artifact(&lower_report).expect("Fix: serialize lower full report");

        assert!(artifact.contains("\"kind\":\"vyre.lower.full_report.json\""));
        assert!(artifact.contains("\"descriptor_id\":\"bench_bad\""));
        assert!(artifact.contains("\"verify_input_status\":\"FAIL\""));
        assert!(artifact.contains("\"verify_output_status\":\"SKIPPED\""));
        assert!(artifact.contains("\"histogram\""));
        assert!(artifact.contains("\"rewrite_stats\""));
        assert!(artifact.contains("\"full_report\""));
        assert!(
            artifact.contains("\"fix_text\":\"Fix:"),
            "Fix: invalid lower descriptors must put repair text in benchmark artifacts."
        );

        let mut case = case_report(
            "failed",
            Correctness::Invalid {
                reason: lower_report.fix_text.clone(),
            },
            None,
        );
        case.artifacts.push(artifact);
        let serialized_case =
            serde_json::to_string(&case).expect("Fix: serialize benchmark case evidence");
        assert!(serialized_case.contains(LOWER_FULL_REPORT_ARTIFACT_KIND));
        assert!(serialized_case.contains("bench_bad"));
        assert!(serialized_case.contains("Fix:"));
    }

    #[test]
    fn benchmark_evidence_schema_rejects_missing_required_metric() {
        let mut case = case_report("pass", Correctness::Exact, Some(performance(true)));
        case.metrics.remove("gpu_digest");
        let error = case
            .validate_benchmark_evidence_schema()
            .expect_err("Fix: missing benchmark evidence metrics must be rejected.");
        assert!(
            error.contains("gpu_digest"),
            "Fix: validation error should name the missing metric; got {error}"
        );
    }

    #[test]
    fn scan_benchmark_metric_evidence_accepts_compile_and_scan_time_split() {
        let evidence = ScanBenchmarkMetricEvidence {
            case_id: "scan.literal_set.irregular_hotloop.4m".to_string(),
            compile_time_ns: 10,
            database_bytes: 4096,
            scratch_bytes: 512,
            cold_throughput_bytes_per_s: 1_000_000,
            warm_throughput_bytes_per_s: 2_000_000,
            streaming_setup_ns: 7,
        };

        validate_scan_benchmark_metric_evidence(&evidence)
            .expect("Fix: complete scan metric evidence must pass");
        assert!(evidence.is_complete());
    }

    #[test]
    fn scan_case_report_requires_scan_specific_metrics() {
        let mut case = case_report("pass", Correctness::Exact, Some(performance(true)));
        case.id = "scan.literal_set.irregular_hotloop.4m".to_string();
        case.tags.push("scan".to_string());
        let error = case
            .validate_benchmark_evidence_schema()
            .expect_err("Fix: scan reports missing scan metrics must reject");
        assert!(error.contains("scan_compile_time_ns"));

        for (name, value) in [
            ("scan_compile_time_ns", 10),
            ("scan_database_bytes", 4096),
            ("scan_scratch_bytes", 512),
            ("scan_cold_throughput_bytes_per_s", 1_000_000),
            ("scan_warm_throughput_bytes_per_s", 2_000_000),
            ("scan_streaming_setup_ns", 7),
        ] {
            case.metrics.insert(name.to_string(), stats(value));
        }
        case.validate_benchmark_evidence_schema()
            .expect("Fix: scan report with split compile/scan metrics must pass");
    }

    #[test]
    fn benchmark_device_signature_changes_with_profile_facts() {
        let baseline =
            benchmark_device_signature(vyre_driver::DeviceProfile::conservative("test"));
        let mut tensor = vyre_driver::DeviceProfile::conservative("test");
        tensor.supports_tensor_cores = true;
        assert_ne!(
            baseline,
            benchmark_device_signature(tensor),
            "Fix: report device signatures must reflect DeviceProfile facts used by planners."
        );
    }

    #[test]
    fn backend_profile_projects_timing_quality_for_reports() {
        let mut profile = vyre_driver::DeviceProfile::conservative("metal");
        profile.timing_quality = vyre_driver::DeviceTimingQuality::HostEnqueueWait;
        profile.supports_device_timestamps = false;
        profile.supports_hardware_counters = false;
        profile.supports_subgroup_ops = true;
        profile.max_workgroup_size = [1024, 1, 1];
        profile.max_invocations_per_workgroup = 1024;
        profile.max_storage_buffer_binding_size = 1 << 30;

        let report_profile = ReportBackendProfile::from_device_profile(profile);

        assert_eq!(report_profile.backend, "metal");
        assert_eq!(report_profile.timing_quality, "host_enqueue_wait");
        assert!(!report_profile.supports_device_timestamps);
        assert!(!report_profile.supports_hardware_counters);
        assert!(report_profile.supports_subgroup_ops);
        assert_eq!(report_profile.max_workgroup_size, [1024, 1, 1]);
        assert_eq!(report_profile.max_invocations_per_workgroup, 1024);
        assert_eq!(report_profile.max_storage_buffer_binding_size, 1 << 30);
    }

    #[test]
    fn summary_pass_requires_pass_status_valid_correctness_and_contract() {
        assert!(
            case_report("pass", Correctness::Exact, Some(performance(true)))
                .passes_summary_evidence(),
            "Fix: valid pass evidence should still count as a passed benchmark case."
        );

        for rejected in [
            case_report("failed", Correctness::Exact, Some(performance(true))),
            case_report(
                "pass",
                Correctness::Invalid {
                    reason: "CUDA/WGPU output mismatch at row 17".to_string(),
                },
                Some(performance(true)),
            ),
            case_report("pass", Correctness::Exact, Some(performance(false))),
            case_report("unstable", Correctness::Exact, Some(performance(true))),
            case_report(
                "thermal_unstable",
                Correctness::Exact,
                Some(performance(true)),
            ),
        ] {
            assert!(
                !rejected.passes_summary_evidence(),
                "Fix: summary.passed must not count failed, invalid, contract-failed, or unstable case evidence: {rejected:?}"
            );
        }
    }
}

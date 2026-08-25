//! ROADMAP M2  -  per-op kernel-time table emitter.
//!
//! Lane: `bench_harness`. Op id: `vyre-bench::report::kernel_time_table`.
//!
//! ## What
//!
//! Iterates every `CaseReport` in a `ReportSchema`, emits one
//! pipe-delimited table row per case with columns:
//!
//! - `case_id`
//! - `kernel_execute_ns_p50`
//! - `kernel_execute_ns_p99`
//! - `bytes_touched_p50`
//! - `wall_throughput_gb_s_p50`
//!
//! Cases missing `kernel_execute_ns` in their metrics map are emitted
//! with `MISSING` timing fields so release evidence exposes coverage
//! holes instead of dropping rows.
//! Output is plain text suitable for `column -t -s '|'`.

use crate::report::ReportSchema;
use std::fmt::Write;

const HEADER: &str = "case_id|kernel_execute_ns_p50|kernel_execute_ns_p99|bytes_touched_p50|wall_throughput_gb_s_p50";

/// Emit a pipe-delimited kernel-time table from a `ReportSchema`.
///
/// Returns the full table string including a header line. Cases without
/// `kernel_execute_ns` in their metrics are emitted with explicit
/// `MISSING` timing fields.
#[must_use]
pub fn kernel_time_table(report: &ReportSchema) -> String {
    let mut out = String::with_capacity(128 * (report.cases.len() + 1));
    out.push_str(HEADER);
    out.push('\n');

    for case in &report.cases {
        if let Some(kernel) = case.metrics.get("kernel_execute_ns") {
            write!(out, "{}|{}|{}|", case.id, kernel.p50, kernel.p99).unwrap_or(());
        } else {
            write!(out, "{}|MISSING|MISSING|", case.id).unwrap_or(());
        }
        if let Some(bytes_touched) = case.metrics.get("bytes_touched") {
            write!(out, "{}", bytes_touched.p50).unwrap_or(());
        } else {
            out.push('-');
        }
        out.push('|');
        if let Some(wall_tp) = case.metrics.get("wall_throughput_gb_s") {
            // wall_throughput_gb_s is stored as a u64 in MetricStats but
            // semantically represents f64. We emit the p50 raw value.
            write!(out, "{}", wall_tp.p50).unwrap_or(());
        } else {
            out.push('-');
        }
        out.push('\n');
    }

    out
}

/// Emit a JSON array of objects representing the kernel-time table.
///
/// Returns a JSON array string. Cases without `kernel_execute_ns` in
/// their metrics are emitted with null timing fields.
#[must_use]
pub fn kernel_time_table_json(report: &ReportSchema) -> String {
    let mut entries = Vec::new();
    for case in &report.cases {
        let mut entry = serde_json::json!({
            "case_id": case.id,
            "kernel_execute_ns_p50": case.metrics.get("kernel_execute_ns").map(|kernel| kernel.p50),
            "kernel_execute_ns_p99": case.metrics.get("kernel_execute_ns").map(|kernel| kernel.p99),
        });

        let Some(map) = entry.as_object_mut() else {
            continue;
        };

        if let Some(bytes_touched) = case.metrics.get("bytes_touched") {
            map.insert(
                "bytes_touched_p50".to_string(),
                serde_json::json!(bytes_touched.p50),
            );
        } else {
            map.insert("bytes_touched_p50".to_string(), serde_json::Value::Null);
        }

        if let Some(wall_tp) = case.metrics.get("wall_throughput_gb_s") {
            map.insert(
                "wall_throughput_gb_s_p50".to_string(),
                serde_json::json!(wall_tp.p50),
            );
        } else {
            map.insert(
                "wall_throughput_gb_s_p50".to_string(),
                serde_json::Value::Null,
            );
        }

        entries.push(entry);
    }
    serde_json::Value::Array(entries).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::case::Correctness;
    use crate::api::metric::MetricStats;
    use crate::probes::environment::EnvironmentData;
    use crate::report::{CaseReport, ReportSchema, ReportSummary};
    use std::collections::BTreeMap;

    fn stat(p50: u64, p99: u64) -> MetricStats {
        MetricStats {
            min: p50,
            p50,
            p90: p50,
            p95: p50,
            p99,
            p999: p99,
            p9999: p99,
            max: p99,
            mean: p50 as f64,
            stddev: 0.0,
            samples: 30,
            determinism_cv: None,
        }
    }

    fn case(id: &str, stages: &[(&str, u64, u64)]) -> CaseReport {
        let mut metrics = BTreeMap::new();
        for (k, p50, p99) in stages {
            metrics.insert((*k).to_string(), stat(*p50, *p99));
        }
        CaseReport {
            id: id.to_string(),
            workload_fingerprint: format!("bench-case:{id}"),
            name: id.to_string(),
            owner_crate: "vyre-bench-test".to_string(),
            workload_class: "Micro".to_string(),
            tags: Vec::new(),
            backend_id: Some("test".to_string()),
            device_signature: Some("device-profile-v1:test".to_string()),
            held_out_corpus_id: Some(format!("heldout:bench-case:{id}")),
            needs_gpu: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            required_features: Vec::new(),
            status: "ok".to_string(),
            wall_ns: None,
            correctness: Correctness::Exact,
            contract: None,
            performance: None,
            metrics,
            optimization_passes_applied: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    fn schema(cases: Vec<CaseReport>) -> ReportSchema {
        ReportSchema {
            schema: "vyre-bench/v1".to_string(),
            run_id: "test".to_string(),
            suite: "kernel_time_test".to_string(),
            selected_backend: Some("test".to_string()),
            backend_profile: None,
            git: BTreeMap::new(),
            source_fingerprint: "test-source".to_string(),
            source_tree_fingerprint: "test-source-tree".to_string(),
            environment: EnvironmentData {
                os: "test".to_string(),
                architecture: "x86_64".to_string(),
                cpu_model: Some("test-cpu".to_string()),
                cpu_cores: 1,
                has_gpu: true,
                gpu_devices: vec![crate::probes::environment::GpuDeviceInfo {
                    name: "NVIDIA GeForce RTX 5090".to_string(),
                    driver_version: "test-driver".to_string(),
                    memory_total_mib: Some(32_768),
                    compute_capability_major: Some(12),
                    compute_capability_minor: Some(0),
                }],
                nvidia_driver_version: Some("test-driver".to_string()),
                nvidia_cuda_version: Some("test-cuda".to_string()),
                features: vec!["gpu.nvidia_smi".to_string()],
            },
            features: Vec::new(),
            cases,
            summary: ReportSummary {
                total_cases: 0,
                passed: 0,
                failed: 0,
                total_time_ns: 0,
                cache_hit_rate: None,
            },
            blockers: Vec::new(),
        }
    }

    #[test]
    fn multi_case_table() {
        let report = schema(vec![
            case(
                "op_a",
                &[
                    ("kernel_execute_ns", 1000, 2000),
                    ("bytes_touched", 4096, 4096),
                    ("wall_throughput_gb_s", 10, 10),
                ],
            ),
            case("op_b", &[("kernel_execute_ns", 500, 800)]),
        ]);
        let table = kernel_time_table(&report);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 data rows");
        assert!(lines[0].starts_with("case_id|"));
        assert!(lines[1].starts_with("op_a|1000|2000|4096|10"));
        assert!(lines[2].starts_with("op_b|500|800|-|-"));
    }

    #[test]
    fn single_case_table() {
        let report = schema(vec![case("single", &[("kernel_execute_ns", 999, 1500)])]);
        let table = kernel_time_table(&report);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].starts_with("single|999|1500"));
    }

    #[test]
    fn missing_kernel_execute_emits_explicit_missing_fields() {
        let report = schema(vec![case("no_kernel", &[("optimize_ns", 100, 200)])]);
        let table = kernel_time_table(&report);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "case without kernel_execute_ns must remain visible"
        );
        assert!(lines[1].starts_with("no_kernel|MISSING|MISSING|-|-"));
    }

    #[test]
    fn empty_report_only_header() {
        let report = schema(vec![]);
        let table = kernel_time_table(&report);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 1, "only header for empty report");
    }

    #[test]
    fn kernel_time_table_json_multi_case() {
        let report = schema(vec![
            case(
                "op_a",
                &[
                    ("kernel_execute_ns", 1000, 2000),
                    ("bytes_touched", 4096, 4096),
                    ("wall_throughput_gb_s", 10, 10),
                ],
            ),
            case("op_b", &[("kernel_execute_ns", 500, 800)]),
        ]);
        let out = kernel_time_table_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["case_id"], "op_a");
        assert_eq!(arr[0]["kernel_execute_ns_p50"], 1000);
        assert_eq!(arr[0]["kernel_execute_ns_p99"], 2000);
        assert_eq!(arr[0]["bytes_touched_p50"], 4096);
        assert_eq!(arr[0]["wall_throughput_gb_s_p50"], 10);

        assert_eq!(arr[1]["case_id"], "op_b");
        assert_eq!(arr[1]["kernel_execute_ns_p50"], 500);
        assert!(arr[1]["bytes_touched_p50"].is_null());
    }

    #[test]
    fn kernel_time_table_json_single_case() {
        let report = schema(vec![case("single", &[("kernel_execute_ns", 999, 1500)])]);
        let out = kernel_time_table_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["case_id"], "single");
    }

    #[test]
    fn kernel_time_table_json_missing_kernel_execute_emits_nulls() {
        let report = schema(vec![case("no_kernel", &[("optimize_ns", 100, 200)])]);
        let out = kernel_time_table_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["case_id"], "no_kernel");
        assert!(arr[0]["kernel_execute_ns_p50"].is_null());
        assert!(arr[0]["kernel_execute_ns_p99"].is_null());
    }

    #[test]
    fn kernel_time_table_json_empty_report() {
        let report = schema(vec![]);
        let out = kernel_time_table_json(&report);
        assert_eq!(out, "[]");
    }
}

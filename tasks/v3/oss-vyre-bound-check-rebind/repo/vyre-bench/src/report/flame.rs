//! ROADMAP M0  -  per-stage flame-graph emitter for the warm-batch corpus.
//!
//! Lane: `bench_harness`. Op id:
//! `vyre-bench::report::flame`. Soundness: read-only over a finished
//! [`ReportSchema`]; never mutates the underlying samples.
//!
//! ## What
//!
//! Every `CaseReport` aggregates per-stage timing samples in
//! `metrics: BTreeMap<String, MetricStats>`  -  keyed by the field
//! names from `crate::api::metric::BenchMetrics` (`compile_ns`,
//! `validate_ns`, `optimize_ns`, `lower_ns`, `cache_lookup_ns`,
//! `upload_ns`, `dispatch_ns`, `kernel_queue_submit_ns`,
//! `kernel_execute_ns`, `device_sync_ns`, `readback_ns`, `verify_ns`).
//! This module turns those medians into the `flamegraph.pl` /
//! `inferno-flamegraph` collapsed-stack format so every recorded
//! warm-batch run can be visualised without running the bench again.
//!
//! Output line shape:
//! ```text
//! vyre;<case_id>;<stage> <p50_ns>
//! ```
//!
//! Example:
//! ```text
//! vyre;vyre-libs::nn::softmax;optimize 12345
//! vyre;vyre-libs::nn::softmax;lower 8765
//! vyre;vyre-libs::nn::softmax;dispatch 23890
//! vyre;vyre-libs::nn::softmax;kernel_execute 412345
//! ```
//!
//! Pipe into `inferno-flamegraph` to render an SVG:
//! ```text
//! ./cargo_full run -p vyre-bench --release -- ... | inferno-flamegraph > flame.svg
//! ```
//!
//! Why p50 and not mean: the bench harness already collects
//! `MetricStats::{min, p50, p90, p95, p99, max, mean}`. p50 is the
//! flame-graph convention because it is robust to a single GC pause
//! and matches the common "time the median request spent here"
//! mental model the SVG visualises.

use crate::report::ReportSchema;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::io;

/// The full set of stage keys we look for in `CaseReport.metrics`. The
/// emitter walks them in order so the resulting flame-graph stacks
/// have a predictable left-to-right order matching the dispatch-time
/// pipeline (compile → validate → optimize → lower → cache lookup →
/// upload → dispatch submit → kernel execute → device sync →
/// readback → verify). Stages with no recorded sample are skipped.
const STAGE_KEYS_ORDERED: &[&str] = &[
    "compile_ns",
    "validate_ns",
    "optimize_ns",
    "lower_ns",
    "cache_lookup_ns",
    "upload_ns",
    "dispatch_ns",
    "kernel_queue_submit_ns",
    "kernel_execute_ns",
    "device_sync_ns",
    "readback_ns",
    "verify_ns",
];

/// Emit one collapsed-stack line per (case, stage) pair where the
/// `MetricStats::p50` value is recorded and non-zero. Returns the
/// concatenated text ready for `inferno-flamegraph`.
#[must_use]
pub fn collapse_report(report: &ReportSchema) -> String {
    let mut out = String::with_capacity(256 * report.cases.len());
    for case in &report.cases {
        write_case_stacks(&mut out, &case.id, &case.metrics);
    }
    out
}

/// Stream the same collapsed-stack output into an arbitrary writer
/// (file, stdout, network sink). Returns the number of stack lines
/// emitted so callers can gate on coverage (e.g. require ≥ 1 stage
/// per case before declaring the corpus M0-ready).
///
/// # Errors
///
/// Returns the underlying writer error verbatim  -  the function does
/// not retry, swallow, or transform write failures.
pub fn write_collapsed<W: io::Write>(report: &ReportSchema, writer: &mut W) -> io::Result<usize> {
    let text = collapse_report(report);
    let lines = text.lines().count();
    writer.write_all(text.as_bytes())?;
    Ok(lines)
}

fn write_case_stacks(
    out: &mut String,
    case_id: &str,
    metrics: &BTreeMap<String, crate::api::metric::MetricStats>,
) {
    for stage in STAGE_KEYS_ORDERED {
        let Some(stats) = metrics.get(*stage) else {
            continue;
        };
        if stats.p50 == 0 {
            continue;
        }
        let display_stage = stage.strip_suffix("_ns").unwrap_or(stage);
        let _ = writeln!(out, "vyre;{};{} {}", case_id, display_stage, stats.p50);
    }
}

/// Emit one collapsed-stack JSON object per (case, stage) pair where the
/// `MetricStats::p50` value is recorded and non-zero. Returns a JSON array string.
#[must_use]
pub fn collapse_report_json(report: &ReportSchema) -> String {
    let mut entries = Vec::new();
    for case in &report.cases {
        for stage in STAGE_KEYS_ORDERED {
            let Some(stats) = case.metrics.get(*stage) else {
                continue;
            };
            if stats.p50 == 0 {
                continue;
            }
            let display_stage = stage.strip_suffix("_ns").unwrap_or(stage);
            entries.push(serde_json::json!({
                "case": case.id,
                "stage": display_stage,
                "p50_ns": stats.p50
            }));
        }
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

    fn stat(p50: u64) -> MetricStats {
        MetricStats {
            min: p50,
            p50,
            p90: p50,
            p95: p50,
            p99: p50,
            p999: p50,
            p9999: p50,
            max: p50,
            mean: p50 as f64,
            stddev: 0.0,
            samples: 30,
            determinism_cv: None,
        }
    }

    fn case(id: &str, stages: &[(&str, u64)]) -> CaseReport {
        let mut metrics = BTreeMap::new();
        for (k, v) in stages {
            metrics.insert((*k).to_string(), stat(*v));
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
            suite: "flame_test".to_string(),
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
    fn collapse_report_emits_one_line_per_recorded_stage() {
        let report = schema(vec![case(
            "vyre-libs::nn::softmax",
            &[("optimize_ns", 100), ("lower_ns", 200), ("dispatch_ns", 50)],
        )]);
        let out = collapse_report(&report);
        // Stage order follows STAGE_KEYS_ORDERED, so optimize before
        // lower before dispatch.
        let expected = "\
vyre;vyre-libs::nn::softmax;optimize 100
vyre;vyre-libs::nn::softmax;lower 200
vyre;vyre-libs::nn::softmax;dispatch 50
";
        assert_eq!(out, expected);
    }

    #[test]
    fn collapse_report_skips_stages_with_zero_p50() {
        let report = schema(vec![case(
            "vyre-libs::nn::softmax",
            &[("optimize_ns", 0), ("lower_ns", 200)],
        )]);
        let out = collapse_report(&report);
        assert_eq!(out, "vyre;vyre-libs::nn::softmax;lower 200\n");
    }

    #[test]
    fn collapse_report_skips_stages_with_no_sample() {
        let report = schema(vec![case("vyre-libs::nn::softmax", &[("optimize_ns", 50)])]);
        let out = collapse_report(&report);
        // Only recorded stages emit flame-graph stacks; absent metrics
        // are intentionally absent from this visualization.
        assert_eq!(out, "vyre;vyre-libs::nn::softmax;optimize 50\n");
    }

    #[test]
    fn collapse_report_handles_multiple_cases_in_input_order() {
        let report = schema(vec![
            case("a", &[("optimize_ns", 10)]),
            case("b", &[("optimize_ns", 20)]),
        ]);
        let out = collapse_report(&report);
        assert_eq!(out, "vyre;a;optimize 10\nvyre;b;optimize 20\n");
    }

    #[test]
    fn collapse_report_emits_no_lines_for_empty_metrics() {
        let report = schema(vec![case("empty", &[])]);
        let out = collapse_report(&report);
        assert!(out.is_empty(), "no stages → no flame-graph stack");
    }

    #[test]
    fn write_collapsed_returns_line_count_and_writes_same_bytes() {
        let report = schema(vec![case(
            "vyre-libs::nn::softmax",
            &[("optimize_ns", 100), ("lower_ns", 200)],
        )]);
        let mut buf = Vec::new();
        let lines = write_collapsed(&report, &mut buf).expect("Fix: write must not fail");
        assert_eq!(lines, 2);
        assert_eq!(String::from_utf8(buf).unwrap(), collapse_report(&report));
    }

    #[test]
    fn collapse_report_json_multi_case() {
        let report = schema(vec![
            case("a", &[("optimize_ns", 10)]),
            case("b", &[("optimize_ns", 20)]),
        ]);
        let out = collapse_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["case"], "a");
        assert_eq!(arr[0]["stage"], "optimize");
        assert_eq!(arr[0]["p50_ns"], 10);
        assert_eq!(arr[1]["case"], "b");
        assert_eq!(arr[1]["p50_ns"], 20);
    }

    #[test]
    fn collapse_report_json_single_case() {
        let report = schema(vec![case(
            "vyre-libs::nn::softmax",
            &[("optimize_ns", 100), ("lower_ns", 200), ("dispatch_ns", 50)],
        )]);
        let out = collapse_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["stage"], "optimize");
        assert_eq!(arr[1]["stage"], "lower");
        assert_eq!(arr[2]["stage"], "dispatch");
    }

    #[test]
    fn collapse_report_json_empty() {
        let report = schema(vec![case("empty", &[])]);
        let out = collapse_report_json(&report);
        assert_eq!(out, "[]");
    }

    #[test]
    fn collapse_report_json_missing_stage() {
        let report = schema(vec![case("vyre-libs::nn::softmax", &[("optimize_ns", 50)])]);
        let out = collapse_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["stage"], "optimize");
    }
}

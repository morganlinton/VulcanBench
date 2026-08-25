use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::benchmark_evidence_semantics::cuda_release_axes_source_artifact_issues;

use super::metrics::{
    max_metric_p50, max_observed_ulp, max_vram_mib, min_first_available_metric_p50, min_metric_p50,
    release_axis_blockers, write_json,
};
use super::suite_inspect::{
    read_text_bounded, record_required_metric_percentile, suite_metric_percentile,
    suite_metric_samples,
};
use super::types::MAX_RELEASE_BENCHMARK_TEXT_BYTES;
use super::types::{
    OptimizationArtifactInspection, OptimizationBenchmarkEvidence, OptimizationBenchmarkManifest,
    OptimizationBenchmarkManifestSummary, ReleaseAxesEvidence,
};

pub(super) fn metric_p50(metric: &Value) -> Option<u64> {
    metric.get("p50").and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn release_axes_uses_cuda_suite_artifacts_only() {
        let dir = TempDir::new().expect("Fix: create temp workspace for release axes source test.");
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let git = vyre_bench::probes::capture_git_info_at(dir.path());
        let source_fingerprint = vyre_bench::probes::source_fingerprint(&git);
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut suite_artifacts = Vec::new();
        let mut artifact_statuses = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            let artifact_path = dir.path().join(&artifact);
            fs::write(
                &artifact_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.axis.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 10 + index},
                                "cold_compile_ns": {"p50": 1000 + index},
                                "wall_gb_s_x1000": {"p50": 2000 + index}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize CUDA release axis fixture."),
            )
            .expect("Fix: write CUDA release axis fixture.");
            artifact_statuses.push(serde_json::json!({
                "path": artifact,
                "family_id": format!("release-axis-{index}"),
                "requested_case_id": format!("release.axis.{index}"),
                "blockers": []
            }));
            suite_artifacts.push(artifact);
        }
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": suite_artifacts,
                "artifact_statuses": artifact_statuses
            }))
            .expect("Fix: serialize CUDA release suite fixture."),
        )
        .expect("Fix: write CUDA release suite fixture.");
        fs::write(
            benchmark_dir.join("wgpu-workload-01.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.axis.wgpu",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"p50": 1},
                            "cold_compile_ns": {"p50": 1},
                            "wall_gb_s_x1000": {"p50": 99_999}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize WGPU release axis decoy."),
        )
        .expect("Fix: write WGPU release axis decoy.");
        fs::write(
            benchmark_dir.join("cpu-only-100x-proof.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.axis.aggregate-proof",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"p50": 1},
                            "cold_compile_ns": {"p50": 1},
                            "wall_gb_s_x1000": {"p50": 99_999}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize aggregate proof release axis decoy."),
        )
        .expect("Fix: write aggregate proof release axis decoy.");

        write_release_axes(dir.path());

        let axes_text = fs::read_to_string(benchmark_dir.join("bench-release-axes.json"))
            .expect("Fix: read generated release axes evidence.");
        let axes = serde_json::from_str::<Value>(&axes_text)
            .expect("Fix: generated release axes evidence must be JSON.");
        let source_artifacts = axes
            .get("source_artifacts")
            .and_then(Value::as_array)
            .expect("Fix: release axes evidence must include source_artifacts.");

        assert_eq!(
            source_artifacts.len(),
            12,
            "Fix: release axes must count CUDA suite workload artifacts, not directory-wide benchmark JSON."
        );
        assert!(
            !source_artifacts.iter().any(|artifact| artifact
                .as_str()
                .is_some_and(|artifact| artifact.contains("wgpu") || artifact.contains("100x"))),
            "Fix: release axes must not include WGPU fallback artifacts or aggregate CPU-SOTA proof reports; source_artifacts={source_artifacts:?}"
        );
        assert_eq!(
            axes.get("warm_us_per_file").and_then(Value::as_f64),
            Some(0.011),
            "Fix: WGPU/proof decoy metrics must not drive CUDA release-axis minima."
        );
        assert!(
            axes.get("blockers")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "Fix: valid CUDA suite artifacts should generate clean release-axis evidence; axes={axes:?}"
        );
    }

    #[test]
    fn release_axes_records_stale_source_provenance_blockers() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for release axes provenance blocker test.");
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let mut suite_artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": "git:abc123:dirty=true",
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.axis.provenance.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 10 + index},
                                "cold_compile_ns": {"p50": 1000 + index},
                                "wall_gb_s_x1000": {"p50": 2000 + index}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize stale provenance release axis fixture."),
            )
            .expect("Fix: write stale provenance release axis fixture.");
            suite_artifacts.push(artifact);
        }
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "artifacts": suite_artifacts
            }))
            .expect("Fix: serialize stale provenance CUDA release suite fixture."),
        )
        .expect("Fix: write stale provenance CUDA release suite fixture.");

        write_release_axes(dir.path());

        let axes_text = fs::read_to_string(benchmark_dir.join("bench-release-axes.json"))
            .expect("Fix: read generated release axes evidence.");
        let axes = serde_json::from_str::<Value>(&axes_text)
            .expect("Fix: generated release axes evidence must be JSON.");
        let blockers = axes
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated release axes evidence must include blockers.");

        assert!(
            blockers.iter().any(|blocker| blocker.as_str().is_some_and(
                |blocker| blocker.contains(
                    "source_artifact `release/evidence/benchmarks/workload-01.json` source_fingerprint `git:abc123:dirty=true` is dirty but has no worktree digest"
                )
            )),
            "Fix: write_release_axes must record source provenance blockers in bench-release-axes instead of leaving them only to the CLI/gate; blockers={blockers:?}"
        );
    }

    #[test]
    fn release_axes_records_mislabeled_cuda_suite_backend_blocker() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for release axes suite backend test.");
        fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let git = vyre_bench::probes::capture_git_info_at(dir.path());
        let source_fingerprint = vyre_bench::probes::source_fingerprint(&git);
        let source_tree_fingerprint = vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut suite_artifacts = Vec::new();
        for index in 1..=12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": &source_fingerprint,
                    "source_tree_fingerprint": &source_tree_fingerprint,
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("release.axis.suite-backend.{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 10 + index},
                                "cold_compile_ns": {"p50": 1000 + index},
                                "wall_gb_s_x1000": {"p50": 2000 + index}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize mislabeled-suite release axis fixture."),
            )
            .expect("Fix: write mislabeled-suite release axis fixture.");
            suite_artifacts.push(artifact);
        }
        fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "wgpu",
                "artifacts": suite_artifacts
            }))
            .expect("Fix: serialize mislabeled CUDA release suite fixture."),
        )
        .expect("Fix: write mislabeled CUDA release suite fixture.");

        write_release_axes(dir.path());

        let axes_text = fs::read_to_string(benchmark_dir.join("bench-release-axes.json"))
            .expect("Fix: read generated release axes evidence.");
        let axes = serde_json::from_str::<Value>(&axes_text)
            .expect("Fix: generated release axes evidence must be JSON.");
        let blockers = axes
            .get("blockers")
            .and_then(Value::as_array)
            .expect("Fix: generated release axes evidence must include blockers.");

        assert!(
            blockers.iter().any(|blocker| blocker.as_str().is_some_and(
                |blocker| blocker.contains(
                    "cuda-release-suite backend `wgpu` does not match required `cuda`"
                )
            )),
            "Fix: write_release_axes must record suite backend identity drift in bench-release-axes; blockers={blockers:?}"
        );
    }

    #[test]
    fn optimization_artifact_inspection_surfaces_backend_and_source_integrity_blockers() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for optimization artifact integrity test.");
        let artifact = "release/evidence/optimization/optimizer-backend-drift.json";
        let artifact_path = dir.path().join(artifact);
        fs::create_dir_all(
            artifact_path
                .parent()
                .expect("Fix: optimization artifact path must have a parent directory."),
        )
        .expect("Fix: create optimization artifact integrity parent directory.");
        fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "foundation.optimizer.impact",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "optimizer_input_nodes": {"p50": 20},
                            "optimizer_output_nodes": {"p50": 5},
                            "optimizer_nodes_eliminated": {"p50": 15},
                            "benchmark_repeats": {"p50": 30}
                        },
                        "contract": {
                            "primitive": "foundation optimizer impact",
                            "baselines": [
                                {
                                    "class": "CpuSota",
                                    "backend_ids": ["cuda"],
                                    "min_speedup_x": 100.0
                                }
                            ]
                        },
                        "performance": {"contract_passed": true, "speedup_x": 100.0}
                    }
                ]
            }))
            .expect("Fix: serialize optimization artifact integrity fixture."),
        )
        .expect("Fix: write optimization artifact integrity fixture.");

        let inspection = inspect_optimization_benchmark_artifact(
            dir.path(),
            "cuda",
            artifact,
            &[
                "optimizer_input_nodes",
                "optimizer_output_nodes",
                "optimizer_nodes_eliminated",
            ],
            &["optimizer_input_nodes", "optimizer_output_nodes"],
        );

        assert!(
            inspection.blockers.iter().any(|blocker| blocker.contains(
                "selected_backend `Some(\"wgpu\")` does not match requested backend `cuda`"
            )),
            "Fix: optimization benchmark inspection must expose backend identity drift; blockers={:?}",
            inspection.blockers
        );
        assert!(
            inspection.blockers.iter().any(|blocker| blocker.contains(
                "source_artifact `release/evidence/optimization/optimizer-backend-drift.json` case `foundation.optimizer.impact` backend `wgpu` has no applicable performance contract baseline"
            )),
            "Fix: optimization benchmark inspection must expose wrong-backend performance contracts; blockers={:?}",
            inspection.blockers
        );
        let borrowed_artifact = "release/evidence/optimization/optimizer-borrowed-cuda.json";
        let borrowed_path = dir.path().join(borrowed_artifact);
        fs::write(
            &borrowed_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "foundation.optimizer.impact",
                        "backend_id": "cuda",
                        "status": "pass",
                        "optimization_passes_applied": ["cuda-resident-borrowed-escape-hatch"],
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "optimizer_input_nodes": {"p50": 20},
                            "optimizer_output_nodes": {"p50": 5},
                            "optimizer_nodes_eliminated": {"p50": 15},
                            "cuda_resident_borrowed_fallback_dispatches": {"p50": 2.0}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 100.0}
                    }
                ]
            }))
            .expect("Fix: serialize borrowed CUDA optimization artifact fixture."),
        )
        .expect("Fix: write borrowed CUDA optimization artifact fixture.");

        let borrowed_inspection = inspect_optimization_benchmark_artifact(
            dir.path(),
            "cuda",
            borrowed_artifact,
            &[
                "optimizer_input_nodes",
                "optimizer_output_nodes",
                "optimizer_nodes_eliminated",
            ],
            &["optimizer_input_nodes", "optimizer_output_nodes"],
        );

        assert!(
            borrowed_inspection.blockers.iter().any(|blocker| {
                blocker.contains(
                    "source_artifact `release/evidence/optimization/optimizer-borrowed-cuda.json` case `foundation.optimizer.impact` has cuda_resident_borrowed_fallback_dispatches p50=2",
                ) && blocker.contains("optimization benchmark `release/evidence/optimization/optimizer-borrowed-cuda.json` must use native resident dispatch")
            }),
            "Fix: optimization benchmark inspection must expose borrowed resident CUDA dispatch telemetry; blockers={:?}",
            borrowed_inspection.blockers
        );
    }
}

pub(super) fn inspect_optimization_benchmark_artifact(
    workspace_root: &Path,
    expected_backend: &str,
    artifact: &str,
    required_custom_metrics: &[&str],
    required_positive_metrics: &[&str],
) -> OptimizationArtifactInspection {
    let mut blockers = Vec::new();
    let path = workspace_root.join(artifact);
    let (exists, mut read_error) = match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => (true, None),
        Ok(metadata) if metadata.is_file() => {
            blockers.push("empty".to_string());
            (true, None)
        }
        Ok(_) => {
            blockers.push("not a file".to_string());
            (false, None)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            blockers.push("missing".to_string());
            (false, Some(error.to_string()))
        }
        Err(error) => {
            let message = error.to_string();
            blockers.push(format!("unreadable metadata: {error}"));
            (false, Some(message))
        }
    };
    if !blockers.is_empty() {
        return OptimizationArtifactInspection {
            exists,
            read_error,
            selected_backend: None,
            source_fingerprint: None,
            source_tree_fingerprint: None,
            environment: None,
            summary_total_time_ns: None,
            summary_cache_hit_rate: None,
            case_count: 0,
            min_wall_samples: None,
            min_wall_p50: None,
            min_wall_p95: None,
            min_wall_p99: None,
            min_baseline_wall_samples: None,
            min_baseline_wall_p50: None,
            min_baseline_wall_p95: None,
            min_baseline_wall_p99: None,
            min_wall_speedup_x1000: None,
            missing_custom_metrics: required_custom_metrics
                .iter()
                .map(|metric| (*metric).to_string())
                .collect(),
            non_positive_required_metrics: required_positive_metrics
                .iter()
                .map(|metric| (*metric).to_string())
                .collect(),
            non_winning_cases: Vec::new(),
            blockers,
        };
    }
    let text = match read_text_bounded(&path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
        Ok(text) => text,
        Err(error) => {
            read_error = Some(error.to_string());
            blockers.push(format!("unreadable JSON: {error}"));
            String::new()
        }
    };
    let report = if text.is_empty() {
        Value::Null
    } else {
        match serde_json::from_str::<Value>(&text) {
            Ok(report) => report,
            Err(error) => {
                blockers.push(format!("invalid JSON: {error}"));
                Value::Null
            }
        }
    };
    if report.get("selected_backend").and_then(Value::as_str) != Some(expected_backend) {
        blockers.push(format!(
            "selected_backend `{:?}` does not match requested backend `{expected_backend}`",
            report.get("selected_backend").and_then(Value::as_str)
        ));
    }
    let selected_backend = report
        .get("selected_backend")
        .and_then(Value::as_str)
        .map(str::to_string);
    let source_fingerprint = report
        .get("source_fingerprint")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let source_tree_fingerprint = report
        .get("source_tree_fingerprint")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let environment = report.get("environment").cloned();
    let summary_total_time_ns = report
        .get("summary")
        .and_then(|summary| summary.get("total_time_ns"))
        .and_then(Value::as_u64);
    let summary_cache_hit_rate = report
        .get("summary")
        .and_then(|summary| summary.get("cache_hit_rate"))
        .and_then(Value::as_f64);
    crate::benchmark_evidence_semantics::inspect_source_artifact_case_integrity(
        artifact,
        &report,
        &format!("optimization benchmark `{artifact}`"),
        &mut blockers,
    );
    let cases = report
        .get("cases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if cases.is_empty() {
        blockers.push("cases array is empty or missing".to_string());
    }
    let mut min_wall_samples = None::<u64>;
    let mut min_baseline_wall_samples = None::<u64>;
    let mut min_wall_p50 = None::<u64>;
    let mut min_wall_p95 = None::<u64>;
    let mut min_wall_p99 = None::<u64>;
    let mut min_baseline_wall_p50 = None::<u64>;
    let mut min_baseline_wall_p95 = None::<u64>;
    let mut min_baseline_wall_p99 = None::<u64>;
    let mut min_wall_speedup_x1000 = None::<u64>;
    let mut missing_custom_metrics = Vec::new();
    let mut non_positive_required_metrics = Vec::new();
    let mut non_winning_cases = Vec::new();
    for case in &cases {
        let case_id = case
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let metrics = case.get("metrics").and_then(Value::as_object);
        let wall_samples = metrics
            .and_then(|metrics| suite_metric_samples(metrics.get("wall_ns")))
            .unwrap_or(0);
        min_wall_samples = Some(min_wall_samples.map_or(wall_samples, |min| min.min(wall_samples)));
        if wall_samples < 30 {
            blockers.push(format!(
                "case `{case_id}` has {wall_samples} wall_ns sample(s), needs at least 30"
            ));
        }
        let baseline_wall_samples = metrics
            .and_then(|metrics| suite_metric_samples(metrics.get("baseline_wall_ns")))
            .unwrap_or(0);
        min_baseline_wall_samples = Some(
            min_baseline_wall_samples
                .map_or(baseline_wall_samples, |min| min.min(baseline_wall_samples)),
        );
        if baseline_wall_samples < 30 {
            blockers.push(format!(
                "case `{case_id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30"
            ));
        }
        record_required_metric_percentile(
            &mut min_wall_p50,
            metrics,
            "wall_ns",
            "p50",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_wall_p95,
            metrics,
            "wall_ns",
            "p95",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_wall_p99,
            metrics,
            "wall_ns",
            "p99",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p50,
            metrics,
            "baseline_wall_ns",
            "p50",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p95,
            metrics,
            "baseline_wall_ns",
            "p95",
            &mut blockers,
            case_id,
        );
        record_required_metric_percentile(
            &mut min_baseline_wall_p99,
            metrics,
            "baseline_wall_ns",
            "p99",
            &mut blockers,
            case_id,
        );
        match (
            metrics.and_then(|metrics| suite_metric_percentile(metrics.get("wall_ns"), "p50")),
            metrics.and_then(|metrics| {
                suite_metric_percentile(metrics.get("baseline_wall_ns"), "p50")
            }),
        ) {
            (Some(wall), Some(baseline)) if wall > 0 && baseline > wall => {
                let speedup_x1000 = baseline.saturating_mul(1_000) / wall;
                min_wall_speedup_x1000 = Some(
                    min_wall_speedup_x1000.map_or(speedup_x1000, |min| min.min(speedup_x1000)),
                );
            }
            (Some(_), Some(_)) if optimization_semantic_win(case_id, metrics) => {}
            (Some(wall), Some(baseline)) => {
                non_winning_cases.push(format!(
                    "{case_id}:wall_p50={wall}:baseline_wall_p50={baseline}"
                ));
            }
            _ => {
                non_winning_cases.push(format!("{case_id}:missing-wall-or-baseline-p50"));
            }
        }
        for metric in required_custom_metrics {
            if !metrics.is_some_and(|metrics| metrics.contains_key(*metric)) {
                missing_custom_metrics.push(format!("{case_id}:{metric}"));
            }
        }
        for metric in required_positive_metrics {
            let positive = metrics
                .and_then(|metrics| metrics.get(*metric))
                .and_then(metric_p50)
                .is_some_and(|value| value > 0);
            if !positive {
                non_positive_required_metrics.push(format!("{case_id}:{metric}"));
            }
        }
    }
    if !missing_custom_metrics.is_empty() {
        blockers.push(format!(
            "missing required metric(s): {}",
            missing_custom_metrics.join(", ")
        ));
    }
    if !non_positive_required_metrics.is_empty() {
        blockers.push(format!(
            "non-positive required metric(s): {}",
            non_positive_required_metrics.join(", ")
        ));
    }
    if !non_winning_cases.is_empty() {
        blockers.push(format!(
            "optimized wall_ns p50 must beat baseline_wall_ns p50 for every case: {}",
            non_winning_cases.join(", ")
        ));
    }
    OptimizationArtifactInspection {
        exists,
        read_error,
        selected_backend,
        source_fingerprint,
        source_tree_fingerprint,
        environment,
        summary_total_time_ns,
        summary_cache_hit_rate,
        case_count: cases.len(),
        min_wall_samples,
        min_wall_p50,
        min_wall_p95,
        min_wall_p99,
        min_baseline_wall_samples,
        min_baseline_wall_p50,
        min_baseline_wall_p95,
        min_baseline_wall_p99,
        min_wall_speedup_x1000,
        missing_custom_metrics,
        non_positive_required_metrics,
        non_winning_cases,
        blockers,
    }
}

pub(super) fn optimization_semantic_win(
    case_id: &str,
    metrics: Option<&serde_json::Map<String, Value>>,
) -> bool {
    let Some(metrics) = metrics else {
        return false;
    };
    match case_id {
        "lower.rewrites.impact.corpus" => {
            suite_metric_percentile(metrics.get("lower_ops_eliminated"), "p50")
                .is_some_and(|value| value > 0)
                || suite_metric_percentile(metrics.get("lower_optimized_issue_score"), "p50")
                    .zip(suite_metric_percentile(
                        metrics.get("lower_baseline_issue_score"),
                        "p50",
                    ))
                    .is_some_and(|(optimized, baseline)| optimized < baseline)
        }
        "foundation.optimizer.impact" => {
            suite_metric_percentile(metrics.get("optimizer_nodes_eliminated"), "p50")
                .is_some_and(|value| value > 0)
        }
        "lower.egraph_saturation" => {
            suite_metric_percentile(metrics.get("egraph_applied_rewrites"), "p50")
                .is_some_and(|value| value > 0)
                && suite_metric_percentile(metrics.get("egraph_output_ops"), "p50")
                    .zip(suite_metric_percentile(
                        metrics.get("egraph_baseline_ops_after"),
                        "p50",
                    ))
                    .is_some_and(|(output, baseline)| output < baseline)
        }
        "lower.alias_aware_optimizations" => {
            suite_metric_percentile(metrics.get("alias_pass_wins"), "p50")
                .is_some_and(|value| value >= 5)
        }
        _ => false,
    }
}

pub(super) fn write_release_axes(workspace_root: &Path) {
    let suite_path = workspace_root.join("release/evidence/benchmarks/cuda-release-suite.json");
    let mut reports = Vec::new();
    let mut source_artifacts = Vec::new();
    let mut blockers = Vec::new();
    let suite_text = match read_text_bounded(&suite_path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "failed to read CUDA release suite `{}`: {error}",
                suite_path.display()
            ));
            String::new()
        }
    };
    let suite = if suite_text.is_empty() {
        Value::Null
    } else {
        match serde_json::from_str::<Value>(&suite_text) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(format!(
                    "invalid CUDA release suite JSON `{}`: {error}",
                    suite_path.display()
                ));
                Value::Null
            }
        }
    };
    let suite_artifacts = suite
        .get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if suite_artifacts.is_empty() {
        blockers.push("CUDA release suite has no artifacts for release axes".to_string());
    }
    for artifact in suite_artifacts {
        let Some(artifact) = artifact
            .as_str()
            .filter(|artifact| !artifact.trim().is_empty())
        else {
            blockers.push("CUDA release suite has a non-string or blank artifact path".to_string());
            continue;
        };
        let path = workspace_root.join(artifact);
        let text = match read_text_bounded(&path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!(
                    "failed to read CUDA release axis artifact `{artifact}`: {error}"
                ));
                continue;
            }
        };
        let value = match serde_json::from_str::<Value>(&text) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(format!(
                    "invalid CUDA release axis artifact JSON `{artifact}`: {error}"
                ));
                continue;
            }
        };
        if value.get("selected_backend").and_then(Value::as_str) != Some("cuda") {
            blockers.push(format!(
                "CUDA release axis artifact `{artifact}` was not produced for cuda"
            ));
            continue;
        }
        if value.get("cases").and_then(Value::as_array).is_none() {
            blockers.push(format!(
                "CUDA release axis artifact `{artifact}` has no cases array"
            ));
            continue;
        }
        if value
            .get("summary")
            .and_then(|summary| summary.get("failed"))
            .and_then(Value::as_u64)
            != Some(0)
        {
            blockers.push(format!(
                "CUDA release axis artifact `{artifact}` has nonzero or missing summary.failed"
            ));
            continue;
        }
        source_artifacts.push(artifact.to_string());
        reports.push(value);
    }
    let warm_us_per_file = min_metric_p50(&reports, "wall_ns").map(|ns| ns as f64 / 1_000.0);
    let cold_pipeline_build_ms = min_first_available_metric_p50(
        &reports,
        &[
            "cold_compile_ns",
            "cold_wall_ns",
            "compile_ns",
            "lower_ns",
            "optimize_ns",
        ],
    )
    .map(|ns| ns as f64 / 1_000_000.0);
    let gbs_scan_throughput = max_metric_p50(&reports, "wall_gb_s_x1000")
        .or_else(|| max_metric_p50(&reports, "device_gb_s_x1000"))
        .map(|gb_s_x1000| gb_s_x1000 as f64 / 1_000.0);
    let ulp_drift_max = Some(max_observed_ulp(&reports).unwrap_or(0));
    let max_vram_mib = max_vram_mib(&reports);
    blockers.extend(release_axis_blockers(&reports));
    blockers.extend(cuda_release_axes_source_artifact_issues(
        workspace_root,
        &json!({
            "warm_us_per_file": warm_us_per_file,
            "cold_pipeline_build_ms": cold_pipeline_build_ms,
            "gbs_scan_throughput": gbs_scan_throughput,
            "ulp_drift_max": ulp_drift_max,
            "max_vram_mib": max_vram_mib,
            "source_artifacts": &source_artifacts
        }),
        &suite,
    ));
    let evidence = ReleaseAxesEvidence {
        schema_version: 1,
        warm_us_per_file,
        cold_pipeline_build_ms,
        gbs_scan_throughput,
        ulp_drift_max,
        max_vram_mib,
        source_artifacts,
        blockers,
    };
    write_json(
        &workspace_root.join("release/evidence/benchmarks/bench-release-axes.json"),
        &evidence,
    );
}

pub(super) fn write_optimization_benchmark_manifest(workspace_root: &Path, backend: &str) {
    let specs = [
        (
            "lower.rewrites.impact.corpus",
            "release/evidence/optimization/lower-rewrite-impact-before-after.json",
            vec![
                "memory-layout",
                "control-flow",
                "vector-layout",
                "A13-coalesce-fixture",
                "A14-shared-mem-promote-fixture",
                "A15-bank-conflict-fixture",
                "A16-vec-pack-fixture",
            ],
            vec![
                "lower_ops_before",
                "lower_ops_after",
                "lower_ops_eliminated",
                "lower_coalesce_problematic_before",
                "lower_shared_candidates_before",
                "lower_bank_critical_before",
                "lower_vec_pack_chains_before",
                "lower_vec_pack_ops_eliminable_before",
            ],
            vec![
                "lower_ops_before",
                "lower_ops_eliminated",
                "lower_coalesce_problematic_before",
                "lower_shared_candidates_before",
                "lower_bank_critical_before",
                "lower_vec_pack_chains_before",
                "lower_vec_pack_ops_eliminable_before",
            ],
        ),
        (
            "foundation.optimizer.impact",
            "release/evidence/optimization/optimizer-impact-cuda.json",
            vec!["algebraic", "predicate"],
            vec![
                "optimizer_input_nodes",
                "optimizer_output_nodes",
                "optimizer_nodes_eliminated",
            ],
            vec!["optimizer_input_nodes", "optimizer_output_nodes"],
        ),
        (
            "lower.egraph_saturation",
            "release/evidence/optimization/egraph-before-after.json",
            vec!["egraph"],
            vec![
                "egraph_case_count",
                "egraph_bitwise_case_count",
                "egraph_boolean_case_count",
                "egraph_equality_classes",
                "egraph_applied_rewrites",
            ],
            vec![
                "egraph_case_count",
                "egraph_bitwise_case_count",
                "egraph_boolean_case_count",
                "egraph_equality_classes",
                "egraph_applied_rewrites",
            ],
        ),
        (
            "lower.alias_aware_optimizations",
            "release/evidence/benchmarks/alias-aware-before-after.json",
            vec![
                "weir-dataflow-dse",
                "weir-dataflow-loop-fusion",
                "weir-dataflow-loop-fission",
                "weir-dataflow-licm",
            ],
            vec![
                "alias_pass_wins",
                "alias_fact_count",
                "alias_cross_binding_fact_count",
                "reaching_def_fact_count",
                "alias_total_ops_after",
                "conservative_total_ops_after",
                "alias_dse_store_count",
                "conservative_dse_store_count",
                "alias_stlf_final_value_id",
                "conservative_stlf_final_value_id",
                "alias_licm_loop_loads",
                "conservative_licm_loop_loads",
                "alias_fusion_loop_count",
                "conservative_fusion_loop_count",
                "alias_fission_loop_count",
                "conservative_fission_loop_count",
                "benchmark_repeats",
            ],
            vec![
                "alias_pass_wins",
                "alias_fact_count",
                "alias_cross_binding_fact_count",
                "reaching_def_fact_count",
                "benchmark_repeats",
            ],
        ),
    ];
    let required_pass_families = vec![
        "algebraic",
        "predicate",
        "egraph",
        "memory-layout",
        "control-flow",
        "vector-layout",
        "A13-coalesce-fixture",
        "A14-shared-mem-promote-fixture",
        "A15-bank-conflict-fixture",
        "A16-vec-pack-fixture",
        "weir-dataflow-dse",
        "weir-dataflow-loop-fusion",
        "weir-dataflow-loop-fission",
        "weir-dataflow-licm",
    ];
    let required_case_count = specs.len();
    let mut blockers = Vec::new();
    let mut covered_pass_families = Vec::new();
    let mut source_fingerprint = None::<String>;
    let mut source_tree_fingerprint = None::<String>;
    let mut environment = None::<Value>;
    let mut total_time_ns = 0u64;
    let mut cache_hit_rates = Vec::new();
    let cases = specs
        .into_iter()
        .map(|(
            case_id,
            artifact,
            pass_families,
            required_custom_metrics,
            required_positive_metrics,
        )| {
            let inspection = inspect_optimization_benchmark_artifact(
                workspace_root,
                backend,
                artifact,
                &required_custom_metrics,
                &required_positive_metrics,
            );
            if !inspection.exists {
                blockers.push(format!(
                    "required optimization benchmark artifact `{artifact}` for `{case_id}` is missing"
                ));
            }
            blockers.extend(inspection.blockers.iter().map(|blocker| {
                format!("optimization benchmark `{case_id}` artifact `{artifact}`: {blocker}")
            }));
            for family in &pass_families {
                covered_pass_families.push(*family);
            }
            if source_fingerprint.is_none() {
                source_fingerprint = inspection.source_fingerprint.clone();
            }
            if source_tree_fingerprint.is_none() {
                source_tree_fingerprint = inspection.source_tree_fingerprint.clone();
            }
            if environment.is_none() {
                environment = inspection.environment.clone();
            }
            total_time_ns =
                total_time_ns.saturating_add(inspection.summary_total_time_ns.unwrap_or(0));
            if let Some(cache_hit_rate) = inspection.summary_cache_hit_rate {
                cache_hit_rates.push(cache_hit_rate);
            }
            let status = if inspection.exists && inspection.blockers.is_empty() {
                "pass"
            } else {
                "failed"
            };
            OptimizationBenchmarkEvidence {
                id: case_id,
                case_id,
                artifact,
                backend_id: inspection
                    .selected_backend
                    .clone()
                    .unwrap_or_else(|| backend.to_string()),
                status: status.to_string(),
                covered_pass_families: pass_families,
                required_custom_metrics,
                required_positive_metrics,
                exists: inspection.exists,
                read_error: inspection.read_error,
                case_count: inspection.case_count,
                min_wall_samples: inspection.min_wall_samples,
                min_wall_p50: inspection.min_wall_p50,
                min_wall_p95: inspection.min_wall_p95,
                min_wall_p99: inspection.min_wall_p99,
                min_baseline_wall_samples: inspection.min_baseline_wall_samples,
                min_baseline_wall_p50: inspection.min_baseline_wall_p50,
                min_baseline_wall_p95: inspection.min_baseline_wall_p95,
                min_baseline_wall_p99: inspection.min_baseline_wall_p99,
                min_wall_speedup_x1000: inspection.min_wall_speedup_x1000,
                missing_custom_metrics: inspection.missing_custom_metrics,
                non_positive_required_metrics: inspection.non_positive_required_metrics,
                non_winning_cases: inspection.non_winning_cases,
                blockers: inspection.blockers,
            }
        })
        .collect::<Vec<_>>();
    covered_pass_families.sort_unstable();
    covered_pass_families.dedup();
    let uncovered_pass_families = required_pass_families
        .iter()
        .copied()
        .filter(|family| !covered_pass_families.contains(family))
        .collect::<Vec<_>>();
    for family in &uncovered_pass_families {
        blockers.push(format!(
            "required optimization pass family `{family}` has no benchmark manifest coverage"
        ));
    }
    let source_fingerprint = source_fingerprint.unwrap_or_else(|| {
        blockers.push("optimization benchmark manifest is missing source_fingerprint".to_string());
        String::new()
    });
    let source_tree_fingerprint = source_tree_fingerprint.unwrap_or_else(|| {
        blockers
            .push("optimization benchmark manifest is missing source_tree_fingerprint".to_string());
        String::new()
    });
    let environment = environment.unwrap_or_else(|| {
        blockers.push("optimization benchmark manifest is missing environment".to_string());
        json!({})
    });
    if environment
        .get("cpu_model")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        blockers.push(
            "optimization benchmark manifest environment is missing cpu_model".to_string(),
        );
    }
    let passed = cases.iter().filter(|case| case.status == "pass").count();
    let failed = cases.len().saturating_sub(passed);
    let cache_hit_rate = if cache_hit_rates.is_empty() {
        None
    } else {
        Some(cache_hit_rates.iter().sum::<f64>() / cache_hit_rates.len() as f64)
    };
    write_json(
        &workspace_root.join("release/evidence/optimization/pass-family-benchmark-manifest.json"),
        &OptimizationBenchmarkManifest {
            schema_version: 2,
            backend: backend.to_string(),
            selected_backend: backend.to_string(),
            source_fingerprint,
            source_tree_fingerprint,
            environment,
            summary: OptimizationBenchmarkManifestSummary {
                total_cases: cases.len(),
                passed,
                failed,
                total_time_ns,
                cache_hit_rate,
            },
            required_case_count,
            required_pass_families,
            covered_pass_families,
            uncovered_pass_families,
            cases,
            blockers,
        },
    );
}

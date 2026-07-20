use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::suite_inspect::{read_text_bounded, suite_metric_percentile, suite_metric_samples};
use super::types::MAX_RELEASE_BENCHMARK_TEXT_BYTES;

pub(super) fn run_named_benchmark(
    workspace_root: &Path,
    case_id: &str,
    backend: &str,
    output: &str,
    measured_samples: Option<usize>,
    sample_timeout_secs: u64,
) {
    let mut owned_args = vec![
        "run".to_string(),
        "-p".to_string(),
        "vyre-bench".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "run".to_string(),
        "--suite".to_string(),
        "release".to_string(),
        "--case".to_string(),
        case_id.to_string(),
        "--backend".to_string(),
        backend.to_string(),
        "--enforce-budgets".to_string(),
        "--output".to_string(),
        output.to_string(),
        "--sample-timeout-secs".to_string(),
        sample_timeout_secs.to_string(),
    ];
    if let Some(samples) = measured_samples {
        owned_args.push("--measured-samples".to_string());
        owned_args.push(samples.to_string());
    }
    let borrowed = owned_args.iter().map(String::as_str).collect::<Vec<_>>();
    run_command(workspace_root, &borrowed);
}

pub(super) fn run_named_benchmark_if_needed(
    workspace_root: &Path,
    case_id: &str,
    backend: &str,
    output: &str,
    measured_samples: Option<usize>,
    sample_timeout_secs: u64,
    reuse_existing: bool,
) {
    if reuse_existing
        && benchmark_artifact_is_reusable(workspace_root, backend, case_id, case_id, output, None)
    {
        return;
    }
    run_named_benchmark(
        workspace_root,
        case_id,
        backend,
        output,
        measured_samples,
        sample_timeout_secs,
    );
}

pub(super) fn benchmark_artifact_is_reusable(
    workspace_root: &Path,
    backend: &str,
    family_id: &str,
    case_id: &str,
    output: &str,
    required_cpu_sota_min_speedup: Option<f64>,
) -> bool {
    let path = workspace_root.join(output);
    let text = match read_text_bounded(&path, MAX_RELEASE_BENCHMARK_TEXT_BYTES) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let Ok(report) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let Some(report_source_fingerprint) = report
        .get("source_fingerprint")
        .and_then(Value::as_str)
        .filter(|fingerprint| !fingerprint.trim().is_empty())
    else {
        return false;
    };
    if !crate::benchmark_evidence_semantics::source_fingerprint_issues(report_source_fingerprint)
        .is_empty()
    {
        return false;
    }
    let Some(report_source_tree_fingerprint) = report
        .get("source_tree_fingerprint")
        .and_then(Value::as_str)
        .filter(|fingerprint| !fingerprint.trim().is_empty())
    else {
        let current_git = vyre_bench::probes::capture_git_info_at(workspace_root);
        let current_source_fingerprint = vyre_bench::probes::source_fingerprint(&current_git);
        if report_source_fingerprint != current_source_fingerprint {
            return false;
        }
        return benchmark_artifact_report_shape_is_reusable(
            &report,
            backend,
            family_id,
            case_id,
            required_cpu_sota_min_speedup,
        );
    };
    let current_source_tree_fingerprint =
        vyre_bench::probes::source_tree_fingerprint_at(workspace_root);
    if report_source_tree_fingerprint != current_source_tree_fingerprint {
        return false;
    }
    benchmark_artifact_report_shape_is_reusable(
        &report,
        backend,
        family_id,
        case_id,
        required_cpu_sota_min_speedup,
    )
}

fn benchmark_artifact_report_shape_is_reusable(
    report: &Value,
    backend: &str,
    family_id: &str,
    case_id: &str,
    required_cpu_sota_min_speedup: Option<f64>,
) -> bool {
    if report.get("selected_backend").and_then(Value::as_str) != Some(backend) {
        return false;
    }
    if report
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(Value::as_u64)
        != Some(0)
    {
        return false;
    }
    if !crate::benchmark_evidence_semantics::benchmark_report_summary_matches_case_evidence(report)
    {
        return false;
    }
    if !crate::benchmark_evidence_semantics::benchmark_failed_case_summaries(report).is_empty() {
        return false;
    }
    let Some(cases) = report.get("cases").and_then(Value::as_array) else {
        return false;
    };
    if cases.len() != 1 {
        return false;
    }
    let case = &cases[0];
    if case.get("id").and_then(Value::as_str) != Some(case_id) {
        return false;
    }
    if case.get("backend_id").and_then(Value::as_str) != Some(backend) {
        return false;
    }
    if case.get("status").and_then(Value::as_str) != Some("pass") {
        return false;
    }
    if !case_has_reusable_timing_metrics(case) {
        return false;
    }
    if let Some(required_speedup) = required_cpu_sota_min_speedup {
        if !case_has_reusable_cpu_sota_contract(case, backend, required_speedup) {
            return false;
        }
    }
    let _ = family_id;
    true
}

fn case_has_reusable_cpu_sota_contract(case: &Value, backend: &str, required_speedup: f64) -> bool {
    crate::benchmark_evidence_semantics::benchmark_case_has_cpu_sota_contract(
        case,
        Some(backend),
        required_speedup,
    ) && case
        .get("performance")
        .and_then(|performance| performance.get("contract_passed"))
        .and_then(Value::as_bool)
        == Some(true)
        && case
            .get("performance")
            .and_then(|performance| performance.get("speedup_x"))
            .and_then(Value::as_f64)
            .is_some_and(|speedup| speedup >= required_speedup)
        && measured_speedup(case).is_some_and(|speedup| speedup >= required_speedup)
}

fn measured_speedup(case: &Value) -> Option<f64> {
    let metrics = case.get("metrics").and_then(Value::as_object)?;
    let wall = metrics
        .get("wall_ns")
        .and_then(|metric| suite_metric_percentile(Some(metric), "p50"))? as f64;
    let baseline = metrics
        .get("baseline_wall_ns")
        .and_then(|metric| suite_metric_percentile(Some(metric), "p50"))? as f64;
    (wall > 0.0).then_some(baseline / wall)
}

fn case_has_reusable_timing_metrics(case: &Value) -> bool {
    let Some(metrics) = case.get("metrics").and_then(Value::as_object) else {
        return false;
    };
    for metric_name in ["wall_ns", "baseline_wall_ns"] {
        if !metrics
            .get(metric_name)
            .and_then(|metric| suite_metric_samples(Some(metric)))
            .is_some_and(|samples| samples >= 30)
        {
            return false;
        }
        for percentile in ["p50", "p95", "p99"] {
            if !metrics
                .get(metric_name)
                .and_then(|metric| suite_metric_percentile(Some(metric), percentile))
                .is_some_and(|value| value > 0)
            {
                return false;
            }
        }
    }
    true
}

pub(super) fn copy_artifact(workspace_root: &Path, source: &str, target: &str) {
    let source = workspace_root.join(source);
    let target = workspace_root.join(target);
    if let Some(parent) = target.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::copy(&source, &target) {
        eprintln!(
            "Fix: failed to copy `{}` to `{}`: {error}",
            source.display(),
            target.display()
        );
        std::process::exit(1);
    }
}

pub(super) fn run_command(workspace_root: &Path, args: &[&str]) {
    if let Err(message) = run_command_status(workspace_root, args) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

pub(super) fn run_command_status(workspace_root: &Path, args: &[&str]) -> Result<(), String> {
    let runner = cargo_runner(workspace_root);
    let status = Command::new(&runner)
        .args(args)
        .current_dir(workspace_root)
        .status();
    let display = format!("{} {}", runner.display(), args.join(" "));
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("Fix: `{display}` failed with {status}")),
        Err(error) => Err(format!(
            "Fix: failed to run `{display}`: {error}. Set VYRE_CARGO_RUNNER to the bounded workspace cargo wrapper if it is not named `cargo_full`."
        )),
    }
}

pub(super) fn cargo_runner(workspace_root: &Path) -> PathBuf {
    if let Some(runner) = std::env::var_os("VYRE_CARGO_RUNNER") {
        return PathBuf::from(runner);
    }
    let local = workspace_root.join("cargo_full");
    if local.is_file() {
        return local;
    }
    PathBuf::from("cargo_full")
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn wgpu_reuse_accepts_matching_passed_artifact() {
        let dir = TempDir::new().expect("Fix: create temp workspace for WGPU reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-condition.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-condition.json",
                None,
            ),
            "Fix: --reuse-existing should skip valid WGPU fallback artifacts instead of rerunning parity benchmarks."
        );
    }

    #[test]
    fn reuse_prefers_matching_source_tree_fingerprint() {
        let dir = TempDir::new().expect("Fix: create temp workspace for source-tree reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-source-tree.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": "git:stale:dirty=false",
                "source_tree_fingerprint": current_test_source_tree_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-source-tree.json",
                None,
            ),
            "Fix: reusable benchmark evidence should survive evidence-only commit changes via source_tree_fingerprint."
        );
    }

    #[test]
    fn cuda_reuse_rejects_optional_cpu_sota_contract_failure() {
        let dir =
            TempDir::new().expect("Fix: create temp workspace for optional contract reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/workload-16-quantized-linear.json",
            serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "nn.linear_4bit_affine_grouped.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "contract": cpu_sota_contract_json("cuda", 100.0),
                        "performance": {"contract_passed": false, "speedup_x": 99.0},
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "cuda",
                "quantized-linear",
                "nn.linear_4bit_affine_grouped.1m",
                "release/evidence/benchmarks/workload-16-quantized-linear.json",
                Some(100.0),
            ),
            "Fix: --reuse-existing must rerun optional CUDA CPU-SOTA artifacts when their published contract failed."
        );
    }

    #[test]
    fn cuda_reuse_accepts_optional_cpu_sota_contract_with_measured_speedup() {
        let dir =
            TempDir::new().expect("Fix: create temp workspace for optional contract reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/workload-16-quantized-linear.json",
            serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "nn.linear_4bit_affine_grouped.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "contract": cpu_sota_contract_json("cuda", 100.0),
                        "performance": {"contract_passed": true, "speedup_x": 100.0},
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            benchmark_artifact_is_reusable(
                dir.path(),
                "cuda",
                "quantized-linear",
                "nn.linear_4bit_affine_grouped.1m",
                "release/evidence/benchmarks/workload-16-quantized-linear.json",
                Some(100.0),
            ),
            "Fix: --reuse-existing should accept optional CUDA CPU-SOTA artifacts only when contract and measured timing evidence agree."
        );
    }

    #[test]
    fn reuse_rejects_matching_source_tree_with_legacy_dirty_source_fingerprint() {
        let dir =
            TempDir::new().expect("Fix: create temp workspace for dirty source-tree reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-legacy-dirty-source.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": "git:abc123:dirty=true",
                "source_tree_fingerprint": current_test_source_tree_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-legacy-dirty-source.json",
                None,
            ),
            "Fix: --reuse-existing must rerun artifacts whose dirty source_fingerprint lacks a worktree digest even when source_tree_fingerprint matches."
        );
    }

    #[test]
    fn reuse_rejects_matching_source_tree_without_source_fingerprint() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for missing source provenance reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-missing-source-fingerprint.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_tree_fingerprint": current_test_source_tree_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-missing-source-fingerprint.json",
                None,
            ),
            "Fix: --reuse-existing must rerun artifacts that cannot satisfy backend suite source_fingerprint provenance."
        );
    }

    #[test]
    fn wgpu_reuse_rejects_backend_or_case_drift() {
        let dir = TempDir::new().expect("Fix: create temp workspace for WGPU reuse drift test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-with-cuda-backend.json",
            serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"}
                ]
            }),
        );
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-wrong-case.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.other.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-with-cuda-backend.json",
                None,
            ),
            "Fix: WGPU reuse must reject artifacts whose selected backend drifted to CUDA."
        );
        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-wrong-case.json",
                None,
            ),
            "Fix: WGPU reuse must reject artifacts that do not contain the requested release case."
        );
    }

    #[test]
    fn reuse_rejects_passed_artifact_without_timing_metrics() {
        let dir =
            TempDir::new().expect("Fix: create temp workspace for missing metrics reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-missing-metrics.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-missing-metrics.json",
                None,
            ),
            "Fix: --reuse-existing must rerun pass-only artifacts that lack release timing metrics."
        );
    }

    #[test]
    fn reuse_rejects_multi_case_artifact_contamination() {
        let dir = TempDir::new().expect("Fix: create temp workspace for multi-case reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-multi-case.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 2, "passed": 2, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    },
                    {
                        "id": "release.entropy_window.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": reusable_timing_metrics()
                    }
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-multi-case.json",
                None,
            ),
            "Fix: --reuse-existing must rerun multi-case artifacts instead of contaminating one-workload backend suite rows."
        );
    }

    #[test]
    fn reuse_rejects_case_failure_hidden_by_summary_zero() {
        let dir =
            TempDir::new().expect("Fix: create temp workspace for hidden failure reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-hidden-failure.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "correctness": {
                            "Invalid": {
                                "reason": "CUDA/WGPU output mismatch at row 17"
                            }
                        }
                    }
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-hidden-failure.json",
                None,
            ),
            "Fix: --reuse-existing must rerun artifacts whose case evidence contradicts summary.failed and pass status."
        );
    }

    #[test]
    fn reuse_rejects_stale_summary_passed_count() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for stale summary passed reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-stale-passed.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 1, "passed": 0, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-stale-passed.json",
                None,
            ),
            "Fix: --reuse-existing must rerun artifacts whose summary.passed contradicts pass-status case evidence."
        );
    }

    #[test]
    fn reuse_rejects_stale_summary_total_cases() {
        let dir = TempDir::new()
            .expect("Fix: create temp workspace for stale summary total_cases reuse test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-stale-total-cases.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "summary": {"total_cases": 2, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-stale-total-cases.json",
                None,
            ),
            "Fix: --reuse-existing must rerun artifacts whose summary.total_cases contradicts the cases array."
        );
    }

    #[test]
    fn reuse_rejects_stale_source_fingerprint() {
        let dir = TempDir::new().expect("Fix: create temp workspace for stale source test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-stale-source.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": "git:stale:dirty=false",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-stale-source.json",
                None,
            ),
            "Fix: --reuse-existing must rerun benchmark artifacts captured from a different source fingerprint."
        );
    }

    #[test]
    fn reuse_rejects_stale_source_tree_fingerprint() {
        let dir = TempDir::new().expect("Fix: create temp workspace for stale source-tree test.");
        write_benchmark_artifact(
            dir.path(),
            "release/evidence/benchmarks/wgpu-stale-source-tree.json",
            serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": current_test_source_fingerprint(dir.path()),
                "source_tree_fingerprint": "source-tree-v1:stale",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {"id": "release.condition_eval.1m", "backend_id": "wgpu", "status": "pass"}
                ]
            }),
        );

        assert!(
            !benchmark_artifact_is_reusable(
                dir.path(),
                "wgpu",
                "condition-eval",
                "release.condition_eval.1m",
                "release/evidence/benchmarks/wgpu-stale-source-tree.json",
                None,
            ),
            "Fix: source_tree_fingerprint must remain a real freshness gate, not only an optional annotation."
        );
    }

    fn current_test_source_fingerprint(workspace_root: &Path) -> String {
        let git = vyre_bench::probes::capture_git_info_at(workspace_root);
        vyre_bench::probes::source_fingerprint(&git)
    }

    fn current_test_source_tree_fingerprint(workspace_root: &Path) -> String {
        vyre_bench::probes::source_tree_fingerprint_at(workspace_root)
    }

    fn reusable_timing_metrics() -> Value {
        serde_json::json!({
            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002}
        })
    }

    fn cpu_sota_contract_json(backend: &str, min_speedup_x: f64) -> Value {
        serde_json::json!({
            "primitive": "fused grouped INT4 linear",
            "baselines": [
                {
                    "name": "Rayon-parallel packed INT4 affine dequantization oracle",
                    "crate_name": "rayon",
                    "class": "CpuSota",
                    "min_speedup_x": min_speedup_x,
                    "backend_ids": [backend]
                }
            ]
        })
    }

    fn write_benchmark_artifact(workspace_root: &Path, relative: &str, value: Value) {
        let path = workspace_root.join(relative);
        fs::create_dir_all(
            path.parent()
                .expect("Fix: benchmark artifact test path must have a parent directory."),
        )
        .expect("Fix: create benchmark artifact test directory.");
        fs::write(&path, format!("{value}\n")).expect("Fix: write benchmark artifact test JSON.");
    }
}

pub(super) struct Config {
    backend: String,
    only: Option<String>,
    measured_samples: Option<usize>,
    sample_timeout_secs: u64,
    include_wgpu_comparison: bool,
    reuse_existing: bool,
}

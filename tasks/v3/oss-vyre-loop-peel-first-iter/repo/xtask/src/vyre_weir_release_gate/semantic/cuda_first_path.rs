use std::path::Path;

use crate::benchmark_evidence_semantics::cuda_release_axes_source_artifact_issues;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(requirement, base_dir, "backend-matrix.json", failures)
    else {
        return;
    };
    let cuda_first = matrix
        .get("cuda_first")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if !cuda_first {
        failures.push(
            "requirement `cuda-first-path` backend matrix does not prove CUDA-first dispatch"
                .to_string(),
        );
    }
    check_backend_matrix_schema("cuda-first-path", &matrix, failures);
    if blockers != 0 {
        failures.push(format!(
            "requirement `cuda-first-path` backend matrix still reports {blockers} blocker(s)"
        ));
    }
    require_no_hidden_backend_fallback_findings("cuda-first-path", &matrix, failures);
    check_backend_gpu_probe("cuda-first-path", &matrix, failures);
    check_preferred_backend_gpu_only("cuda-first-path", &matrix, failures);
    check_backend_acquire_entry("cuda-first-path", &matrix, "cuda", failures);
    check_backend_feature_markers(
        "cuda-first-path",
        &matrix,
        "cuda_feature_markers",
        12,
        failures,
    );
    check_json_evidence_has_no_blockers(requirement, base_dir, "cuda-release-suite.json", failures);
    check_backend_suite_report(requirement, base_dir, "cuda-release-suite.json", failures);
    check_benchmark_report_has_cases(requirement, base_dir, "cuda-ptx-patterns.json", failures);
    check_json_evidence_has_no_blockers(requirement, base_dir, "bench-release-axes.json", failures);
    if let (Some(axes), Some(cuda_suite)) = (
        first_json_evidence(requirement, base_dir, "bench-release-axes.json", failures),
        first_json_evidence(requirement, base_dir, "cuda-release-suite.json", failures),
    ) {
        check_release_axes_source_artifacts(base_dir, &axes, &cuda_suite, failures);
    }
}

fn check_release_axes_source_artifacts(
    base_dir: &Path,
    axes: &serde_json::Value,
    cuda_suite: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let workspace_root = base_dir.parent().unwrap_or(base_dir);
    for issue in cuda_release_axes_source_artifact_issues(workspace_root, axes, cuda_suite) {
        failures.push(format!(
            "requirement `cuda-first-path` bench-release-axes {issue}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_first_axes_counts_only_usable_source_artifacts() {
        let axes = serde_json::json!({
            "source_artifacts": [
                "",
                null,
                "release/evidence/benchmarks/workload-01-condition-eval.json",
                "release/evidence/benchmarks/workload-01-condition-eval.json"
            ]
        });
        let cuda_suite = serde_json::json!({
            "artifacts": ["release/evidence/benchmarks/workload-01-condition-eval.json"]
        });
        let mut failures = Vec::new();

        check_release_axes_source_artifacts(
            Path::new("release"),
            &axes,
            &cuda_suite,
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "bench-release-axes source_artifacts has 1 CUDA workload artifact(s), needs at least 12"
            )),
            "Fix: CUDA-first release axes must not count blank/non-string source_artifacts as evidence; failures={failures:?}"
        );
    }

    #[test]
    fn cuda_first_axes_rejects_missing_source_artifact_files() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for source artifact existence test.");
        let release_dir = dir.path().join("release");
        std::fs::create_dir_all(release_dir.join("evidence/benchmarks"))
            .expect("Fix: create temporary benchmark evidence directory.");
        let mut source_artifacts = Vec::new();
        for index in 0..12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            if index != 11 {
                std::fs::write(dir.path().join(&artifact), "{}")
                    .expect("Fix: write temporary source artifact.");
            }
            source_artifacts.push(artifact);
        }
        let axes = serde_json::json!({
            "source_artifacts": source_artifacts
        });
        let cuda_suite = serde_json::json!({
            "artifacts": source_artifacts
        });
        let mut failures = Vec::new();

        check_release_axes_source_artifacts(&release_dir, &axes, &cuda_suite, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "bench-release-axes source_artifact `release/evidence/benchmarks/workload-11.json` is not a readable file"
            )),
            "Fix: CUDA-first release axes must reject source_artifacts that do not resolve to files; failures={failures:?}"
        );
    }

    #[test]
    fn cuda_first_axes_rejects_source_artifacts_outside_cuda_suite() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for source artifact suite test.");
        let release_dir = dir.path().join("release");
        std::fs::create_dir_all(release_dir.join("evidence/benchmarks"))
            .expect("Fix: create temporary benchmark evidence directory.");
        let mut source_artifacts = Vec::new();
        for index in 0..12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "cases": [{"id": format!("case-{index}"), "status": "pass"}]
                }))
                .expect("Fix: serialize temporary CUDA artifact."),
            )
            .expect("Fix: write temporary CUDA artifact.");
            source_artifacts.push(artifact);
        }
        source_artifacts.push("release/evidence/benchmarks/wgpu-workload-00.json".to_string());
        std::fs::write(
            dir.path()
                .join("release/evidence/benchmarks/wgpu-workload-00.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [{"id": "wgpu-case", "status": "pass"}]
            }))
            .expect("Fix: serialize temporary WGPU artifact."),
        )
        .expect("Fix: write temporary WGPU artifact.");
        let cuda_suite = serde_json::json!({
            "artifacts": source_artifacts
                .iter()
                .filter(|artifact| !artifact.contains("wgpu"))
                .collect::<Vec<_>>()
        });
        let axes = serde_json::json!({
            "source_artifacts": source_artifacts
        });
        let mut failures = Vec::new();

        check_release_axes_source_artifacts(&release_dir, &axes, &cuda_suite, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "source_artifact `release/evidence/benchmarks/wgpu-workload-00.json` is not listed in cuda-release-suite artifacts"
            )),
            "Fix: CUDA-first release axes must reject source artifacts outside the CUDA suite; failures={failures:?}"
        );
    }

    #[test]
    fn cuda_first_axes_rejects_missing_scalar_axes() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for release axis scalar gate test.");
        let release_dir = dir.path().join("release");
        std::fs::create_dir_all(release_dir.join("evidence/benchmarks"))
            .expect("Fix: create temporary benchmark evidence directory.");
        let mut source_artifacts = Vec::new();
        for index in 0..12 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                    "environment": {
                        "gpu_devices": [{"memory_total_mib": 24576}]
                    },
                    "cases": [
                        {
                            "id": format!("case-{index}"),
                            "backend_id": "cuda",
                            "status": "pass",
                            "metrics": {
                                "wall_ns": {"p50": 17_000},
                                "cold_compile_ns": {"p50": 2_000_000},
                                "wall_gb_s_x1000": {"p50": 4_000}
                            },
                            "correctness": {
                                "Toleranced": {"max_observed_ulp": 3}
                            }
                        }
                    ]
                }))
                .expect("Fix: serialize temporary CUDA artifact."),
            )
            .expect("Fix: write temporary CUDA artifact.");
            source_artifacts.push(artifact);
        }
        let axes = serde_json::json!({
            "source_artifacts": source_artifacts
        });
        let cuda_suite = serde_json::json!({
            "backend": "cuda",
            "artifacts": source_artifacts
        });
        let mut failures = Vec::new();

        check_release_axes_source_artifacts(&release_dir, &axes, &cuda_suite, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `cuda-first-path` bench-release-axes bench-release-axes warm_us_per_file is missing or not numeric"
            )),
            "Fix: CUDA-first release gate must reject source-backed axes that omit warm scalar evidence; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "bench-release-axes max_vram_mib is missing or not numeric"
            )),
            "Fix: CUDA-first release gate must reject source-backed axes that omit VRAM scalar evidence; failures={failures:?}"
        );
    }
}

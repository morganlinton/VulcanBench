use crate::benchmark_evidence_semantics::{
    backend_suite_artifact_status_issues, backend_suite_backend_issue,
    backend_suite_inventory_issues, backend_suite_matrix_coverage_issues,
    backend_suite_parity_issues, describe_backend_suite_inventory_issue,
    describe_backend_suite_matrix_coverage_issue,
    expected_backend_for_suite_evidence, BackendSuiteArtifactStatusIssue, BackendSuiteBackendIssue,
    BackendSuiteParityIssue,
};

pub(crate) fn check_backend_suite_report(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    let schema_version = report
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` schema_version={schema_version}; expected schema>=2",
            requirement.id
        ));
    }
    let expected_suite_backend = expected_backend_for_suite_evidence(suffix);
    if let Some(expected_backend) = expected_suite_backend {
        if let Some(issue) = backend_suite_backend_issue(&report, expected_backend) {
            match issue {
                BackendSuiteBackendIssue::Missing { expected_backend } => failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` is missing backend identity `{expected_backend}`",
                    requirement.id
                )),
                BackendSuiteBackendIssue::Mismatch {
                    expected_backend,
                    actual_backend,
                } => failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` backend `{actual_backend}` does not match required `{expected_backend}`",
                    requirement.id
                )),
            }
        }
    }
    let family_count = report
        .get("family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let artifact_count = report
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if family_count == 0 || artifact_count == 0 {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` reports family_count={family_count}, artifacts={artifact_count}",
            requirement.id
        ));
    }
    if family_count < 12 || artifact_count < 12 {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` reports family_count={family_count}, artifacts={artifact_count}; release suites need at least 12 workload families",
            requirement.id
        ));
    }
    match report.get("blockers").and_then(serde_json::Value::as_array) {
        Some(blockers) => {
            for blocker in blockers {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` reports blocker: {}",
                    requirement.id,
                    blocker.as_str().unwrap_or("<non-string blocker>")
                ));
            }
        }
        None => failures.push(format!(
            "requirement `{}` backend suite `{suffix}` is missing blockers array",
            requirement.id
        )),
    }
    for issue in backend_suite_inventory_issues(&report) {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` {}",
            requirement.id,
            describe_backend_suite_inventory_issue(&issue)
        ));
    }
    check_backend_suite_matrix_coverage(requirement, base_dir, suffix, &report, failures);
    if let Some(statuses) = report
        .get("artifact_statuses")
        .and_then(serde_json::Value::as_array)
    {
        for status in statuses {
            let path = status
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if status.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` is missing",
                    requirement.id
                ));
            }
            if status
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` is empty",
                    requirement.id
                ));
            }
            let read_error = status.get("read_error");
            if !read_error.is_some_and(serde_json::Value::is_null) {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` read_error={}",
                    requirement.id,
                    read_error
                        .map(serde_json::Value::to_string)
                        .unwrap_or_else(|| "<missing>".to_string())
                ));
            }
            if status
                .get("family_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` has no family_id",
                    requirement.id
                ));
            }
            if status
                .get("requested_case_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` has no requested_case_id",
                    requirement.id
                ));
            }
            for field in [
                "source_fingerprint",
                "source_tree_fingerprint",
                "host_cpu_model",
            ] {
                if status
                    .get(field)
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|value| value.trim().is_empty())
                {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` artifact `{path}` has no `{field}` provenance",
                        requirement.id
                    ));
                }
            }
            check_backend_suite_status_source_fingerprint_shape(
                requirement,
                suffix,
                path,
                status,
                failures,
            );
            if let (Some((field, source_fingerprint)), Some(current_source_fingerprint)) = (
                report_freshness_fingerprint(status),
                current_freshness_fingerprint_for_report(base_dir, status),
            ) {
                check_source_fingerprint_freshness(
                    requirement,
                    &format!("backend suite `{suffix}` artifact `{path}`"),
                    field,
                    source_fingerprint,
                    &current_source_fingerprint,
                    failures,
                );
            }
            if status
                .get("case_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` reports zero cases",
                    requirement.id
                ));
            }
            if status
                .get("failed_count")
                .and_then(serde_json::Value::as_u64)
                != Some(0)
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` reports nonzero or missing failed_count",
                    requirement.id
                ));
            }
            if status
                .get("nonmatching_case_backend_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1)
                != 0
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` reports backend-mismatched cases",
                    requirement.id
                ));
            }
            let suite_backend = report.get("backend").and_then(serde_json::Value::as_str);
            if matches!(suite_backend, Some("cuda" | "wgpu"))
                && status
                    .get("min_kernel_launches")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` GPU artifact `{path}` has non-positive `min_kernel_launches`",
                    requirement.id
                ));
            }
            if suite_backend == Some("cuda") {
                for field in ["gpu_model", "nvidia_driver_version", "nvidia_cuda_version"] {
                    if status
                        .get(field)
                        .and_then(serde_json::Value::as_str)
                        .is_none_or(|value| value.trim().is_empty())
                    {
                        failures.push(format!(
                            "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` has no `{field}` provenance",
                            requirement.id
                        ));
                    }
                }
                match status
                    .get("gpu_memory_total_mib")
                    .and_then(serde_json::Value::as_u64)
                {
                    Some(mib) if mib >= 16 * 1024 => {}
                    Some(mib) => failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` reports {mib} MiB GPU memory, below release floor 16384 MiB",
                        requirement.id
                    )),
                    None => failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` has no `gpu_memory_total_mib` provenance",
                        requirement.id
                    )),
                }
                match (
                    status
                        .get("gpu_compute_capability_major")
                        .and_then(serde_json::Value::as_u64),
                    status
                        .get("gpu_compute_capability_minor")
                        .and_then(serde_json::Value::as_u64),
                ) {
                    (Some(major), Some(minor)) if (major, minor) >= (8, 0) => {}
                    (Some(major), Some(minor)) => failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` reports compute capability {major}.{minor}, below release floor 8.0",
                        requirement.id
                    )),
                    _ => failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` has no compute capability provenance",
                        requirement.id
                    )),
                }
                if status
                    .get("min_cuda_ptx_source_cache_entries")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
                {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` has non-positive `min_cuda_ptx_source_cache_entries`",
                        requirement.id
                    ));
                }
                for field in [
                    "min_cuda_ptx_source_cache_hits",
                    "min_cuda_ptx_source_cache_misses",
                ] {
                    if status
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .is_none()
                    {
                        failures.push(format!(
                            "requirement `{}` backend suite `{suffix}` CUDA artifact `{path}` is missing `{field}`",
                            requirement.id
                        ));
                    }
                }
            }
            if status
                .get("min_wall_samples")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                < 30
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` has fewer than 30 wall_ns samples",
                    requirement.id
                ));
            }
            if status
                .get("min_baseline_wall_samples")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                < 30
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` has fewer than 30 baseline_wall_ns samples",
                    requirement.id
                ));
            }
            for field in [
                "min_wall_p50",
                "min_wall_p95",
                "min_wall_p99",
                "min_baseline_wall_p50",
                "min_baseline_wall_p95",
                "min_baseline_wall_p99",
            ] {
                if status
                    .get(field)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
                {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` artifact `{path}` has non-positive `{field}`",
                        requirement.id
                    ));
                }
            }
            match status.get("blockers").and_then(serde_json::Value::as_array) {
                Some(blockers) => {
                    for blocker in blockers {
                        failures.push(format!(
                            "requirement `{}` backend suite `{suffix}` artifact `{path}` blocker: {}",
                            requirement.id,
                            blocker.as_str().unwrap_or("<non-string blocker>")
                        ));
                    }
                }
                None => failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` is missing blockers array",
                    requirement.id
                )),
            }
            if status
                .get("cpu_sota_100x_required")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                && status
                    .get("cpu_sota_100x_passing_cases")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` requires CPU-SOTA 100x proof but has zero passing 100x case(s)",
                    requirement.id
                ));
            }
        }
    } else {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` has no artifact_statuses",
            requirement.id
        ));
    }
    if let Some(artifacts) = report
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
    {
        let expected_backend = expected_suite_backend
            .or_else(|| report.get("backend").and_then(serde_json::Value::as_str));
        let workspace_root = base_dir
            .file_name()
            .is_some_and(|name| name == "release")
            .then(|| base_dir.parent())
            .flatten()
            .unwrap_or(base_dir);
        for artifact in artifacts {
            let Some(artifact) = artifact.as_str() else {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` contains a non-string artifact path",
                    requirement.id
                ));
                continue;
            };
            if let Some(issue) =
                crate::benchmark_evidence_semantics::benchmark_suite_artifact_path_issue(
                    workspace_root,
                    artifact,
                )
            {
                failures.push(format!(
                    "requirement `{}` backend suite `{suffix}` {}",
                    requirement.id,
                    issue.describe("suite artifact", artifact)
                ));
                continue;
            }
            let path = resolve_artifact_path(base_dir, artifact);
            let text = match read_text_bounded(&path) {
                Ok(text) => text,
                Err(error) => {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` failed to read listed artifact `{}`: {error}",
                        requirement.id,
                        path.display()
                    ));
                    continue;
                }
            };
            let artifact_report = match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(report) => report,
                Err(error) => {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` listed artifact `{}` is invalid JSON: {error}",
                        requirement.id,
                        path.display()
                    ));
                    continue;
                }
            };
            if let Some(status) = report_status_for_path(&report, artifact) {
                check_backend_suite_artifact_status(
                    requirement,
                    suffix,
                    status,
                    &artifact_report,
                    failures,
                );
            }
            check_single_benchmark_report(
                requirement,
                base_dir,
                &path,
                &artifact_report,
                false,
                None,
                failures,
            );
            if let Some(expected_backend) = expected_backend {
                let selected_backend = artifact_report
                    .get("selected_backend")
                    .and_then(serde_json::Value::as_str);
                if selected_backend != Some(expected_backend) {
                    failures.push(format!(
                        "requirement `{}` backend suite `{suffix}` artifact `{}` selected backend `{:?}`, expected `{expected_backend}`",
                        requirement.id,
                        path.display(),
                        selected_backend
                    ));
                }
                if matches!(expected_backend, "cuda" | "wgpu") {
                    let artifact_label = path.display().to_string();
                    require_case_metric_present(
                        requirement,
                        &artifact_label,
                        &artifact_report,
                        "kernel_launches",
                        failures,
                    );
                    require_case_metric_positive(
                        requirement,
                        &artifact_label,
                        &artifact_report,
                        "kernel_launches",
                        failures,
                    );
                }
                if expected_backend == "cuda" {
                    let artifact_label = path.display().to_string();
                    for metric in [
                        "cuda_ptx_source_cache_entries",
                        "cuda_ptx_source_cache_hits",
                        "cuda_ptx_source_cache_misses",
                    ] {
                        require_case_metric_present(
                            requirement,
                            &artifact_label,
                            &artifact_report,
                            metric,
                            failures,
                        );
                    }
                    for metric in ["cuda_ptx_source_cache_entries"] {
                        require_case_metric_positive(
                            requirement,
                            &artifact_label,
                            &artifact_report,
                            metric,
                            failures,
                        );
                    }
                }
                if let Some(cases) = artifact_report
                    .get("cases")
                    .and_then(serde_json::Value::as_array)
                {
                    for case in cases {
                        let id = case
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("<unknown>");
                        let backend = case.get("backend_id").and_then(serde_json::Value::as_str);
                        if backend != Some(expected_backend) {
                            failures.push(format!(
                                "requirement `{}` backend suite `{suffix}` artifact `{}` case `{id}` backend `{:?}`, expected `{expected_backend}`",
                                requirement.id,
                                path.display(),
                                backend
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn check_backend_suite_matrix_coverage(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    report: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let matrix_path = base_dir.join("evidence/benchmarks/release-workload-matrix.json");
    if !matrix_path.is_file() {
        return;
    }
    let text = match read_text_bounded(&matrix_path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "requirement `{}` backend suite `{suffix}` failed to read release workload matrix `{}`: {error}",
                requirement.id,
                matrix_path.display()
            ));
            return;
        }
    };
    let matrix = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(matrix) => matrix,
        Err(error) => {
            failures.push(format!(
                "requirement `{}` backend suite `{suffix}` release workload matrix `{}` is invalid JSON: {error}",
                requirement.id,
                matrix_path.display()
            ));
            return;
        }
    };
    for issue in backend_suite_matrix_coverage_issues(&matrix, report) {
        failures.push(format!(
            "requirement `{}` backend suite `{suffix}` {}",
            requirement.id,
            describe_backend_suite_matrix_coverage_issue(&issue)
        ));
    }
}

fn check_backend_suite_status_source_fingerprint_shape(
    requirement: &Requirement,
    suffix: &str,
    path: &str,
    status: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let Some(source_fingerprint) = status
        .get("source_fingerprint")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    check_source_fingerprint_shape(
        requirement,
        &format!("backend suite {suffix} status for {path}"),
        source_fingerprint,
        failures,
    );
}

fn report_status_for_path<'a>(
    suite_report: &'a serde_json::Value,
    artifact: &str,
) -> Option<&'a serde_json::Value> {
    suite_report
        .get("artifact_statuses")
        .and_then(serde_json::Value::as_array)
        .and_then(|statuses| {
            statuses.iter().find(|status| {
                status.get("path").and_then(serde_json::Value::as_str) == Some(artifact)
            })
        })
}

fn check_backend_suite_artifact_status(
    requirement: &Requirement,
    suffix: &str,
    status: &serde_json::Value,
    artifact_report: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    for issue in backend_suite_artifact_status_issues(status, artifact_report) {
        match issue {
            BackendSuiteArtifactStatusIssue::MissingField { path, field } => failures.push(
                format!(
                    "requirement `{}` backend suite `{suffix}` artifact `{path}` status is missing artifact-backed `{field}`",
                    requirement.id
                ),
            ),
            BackendSuiteArtifactStatusIssue::SourceFingerprintMismatch {
                path,
                status_source_fingerprint,
                artifact_source_fingerprint,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` source_fingerprint mismatch: status `{status_source_fingerprint}`, artifact `{artifact_source_fingerprint}`",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::SourceTreeFingerprintMismatch {
                path,
                status_source_tree_fingerprint,
                artifact_source_tree_fingerprint,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` source_tree_fingerprint mismatch: status `{status_source_tree_fingerprint}`, artifact `{artifact_source_tree_fingerprint}`",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::SelectedBackendMismatch {
                path,
                status_selected_backend,
                artifact_selected_backend,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` selected_backend mismatch: status `{status_selected_backend}`, artifact `{artifact_selected_backend}`",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::CaseCountMismatch {
                path,
                status_case_count,
                artifact_case_count,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` case_count mismatch: status {status_case_count}, artifact {artifact_case_count}",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::FailedCountMismatch {
                path,
                status_failed_count,
                artifact_failed_count,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` failed_count mismatch: status {status_failed_count}, artifact {artifact_failed_count}",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::NumericFieldMismatch {
                path,
                field,
                status_value,
                artifact_value,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` `{field}` mismatch: status {status_value}, artifact {artifact_value}",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::StringFieldMismatch {
                path,
                field,
                status_value,
                artifact_value,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` `{field}` mismatch: status `{status_value}`, artifact `{artifact_value}`",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::CpuSota100xContractCaseCountMismatch {
                path,
                status_contract_cases,
                artifact_contract_cases,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` cpu_sota_100x_contract_cases mismatch: status {status_contract_cases}, artifact {artifact_contract_cases}",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::CpuSota100xPassingCaseCountMismatch {
                path,
                status_passing_cases,
                artifact_passing_cases,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` cpu_sota_100x_passing_cases mismatch: status {status_passing_cases}, artifact {artifact_passing_cases}",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::MissingRequestedCase {
                path,
                requested_case_id,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` does not contain requested_case_id `{requested_case_id}`",
                requirement.id
            )),
            BackendSuiteArtifactStatusIssue::DuplicateRequestedCase {
                path,
                requested_case_id,
                count,
            } => failures.push(format!(
                "requirement `{}` backend suite `{suffix}` artifact `{path}` contains requested_case_id `{requested_case_id}` {count} times",
                requirement.id
            )),
        }
    }
}

#[cfg(test)]
mod part7_tests {
    use super::*;

    #[test]
    fn backend_suite_status_rejects_dirty_fingerprint_without_worktree_digest() {
        let requirement = Requirement {
            id: "wgpu-fallback".to_string(),
            title: "WGPU fallback".to_string(),
            status: "required".to_string(),
            evidence: Vec::new(),
            minimum_evidence: 0,
        };
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            "source_fingerprint": "git:abc123:dirty=true"
        });
        let mut failures = Vec::new();

        check_backend_suite_status_source_fingerprint_shape(
            &requirement,
            "wgpu-fallback-suite.json",
            "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
            &status,
            &mut failures,
        );

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("is dirty but has no worktree digest")),
            "Fix: backend suite status rows must carry precise dirty-worktree provenance; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_artifact_status_rejects_duplicate_requested_case_rows() {
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: Vec::new(),
            minimum_evidence: 0,
        };
        let status = serde_json::json!({
            "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
            "requested_case_id": "release.condition_eval.1m"
        });
        let artifact = serde_json::json!({
            "cases": [
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"},
                {"id": "release.condition_eval.1m", "backend_id": "cuda", "status": "pass"}
            ]
        });
        let mut failures = Vec::new();

        check_backend_suite_artifact_status(
            &requirement,
            "cuda-release-suite.json",
            &status,
            &artifact,
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "artifact `release/evidence/benchmarks/workload-01-condition-eval.json` contains requested_case_id `release.condition_eval.1m` 2 times"
            )),
            "Fix: release gate must reject suite artifacts where requested_case_id resolves to multiple benchmark rows; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_report_rejects_missing_blockers_array() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for suite blocker array test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for suite blocker array test.");
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 0,
                "artifacts": [],
                "artifact_statuses": []
            }))
            .expect("Fix: serialize CUDA suite without blockers array."),
        )
        .expect("Fix: write CUDA suite without blockers array.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `cuda-first-path` backend suite `cuda-release-suite.json` is missing blockers array"
            )),
            "Fix: backend suite release gate must fail closed when generated suite evidence omits blockers; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_report_rejects_status_missing_blockers_array() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for suite status blocker array test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir).expect(
            "Fix: create benchmark evidence directory for suite status blocker array test.",
        );
        std::fs::write(
            benchmark_dir.join("cuda-workload-01-condition-eval.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "selected_backend": "cuda",
                "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
                "source_tree_fingerprint": "source-tree-v1:test",
                "environment": {"host_cpu_model": "test CPU"},
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_entries": {"samples": 30, "p50": 1},
                            "cuda_ptx_source_cache_hits": {"samples": 30, "p50": 0},
                            "cuda_ptx_source_cache_misses": {"samples": 30, "p50": 1}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    }
                ]
            }))
            .expect("Fix: serialize CUDA benchmark artifact for status blocker array test."),
        )
        .expect("Fix: write CUDA benchmark artifact for status blocker array test.");
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 1,
                "artifacts": ["release/evidence/benchmarks/cuda-workload-01-condition-eval.json"],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-workload-01-condition-eval.json",
                        "exists": true,
                        "bytes": 1,
                        "read_error": null,
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
                        "source_tree_fingerprint": "source-tree-v1:test",
                        "host_cpu_model": "test CPU",
                        "selected_backend": "cuda",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "min_wall_samples": 30,
                        "min_baseline_wall_samples": 30,
                        "min_wall_p50": 10,
                        "min_wall_p95": 11,
                        "min_wall_p99": 12,
                        "min_baseline_wall_p50": 1000,
                        "min_baseline_wall_p95": 1001,
                        "min_baseline_wall_p99": 1002,
                        "min_kernel_launches": 1,
                        "gpu_model": "RTX 5090",
                        "nvidia_driver_version": "580.0",
                        "nvidia_cuda_version": "13.0",
                        "gpu_memory_total_mib": 24576,
                        "gpu_compute_capability_major": 8,
                        "gpu_compute_capability_minor": 9,
                        "min_cuda_ptx_source_cache_entries": 1,
                        "min_cuda_ptx_source_cache_hits": 0,
                        "min_cuda_ptx_source_cache_misses": 1,
                        "cpu_sota_100x_required": false,
                        "cpu_sota_100x_contract_cases": 0,
                        "cpu_sota_100x_passing_cases": 0
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize CUDA suite with status missing blockers array."),
        )
        .expect("Fix: write CUDA suite with status missing blockers array.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "backend suite `cuda-release-suite.json` artifact `release/evidence/benchmarks/cuda-workload-01-condition-eval.json` is missing blockers array"
            )),
            "Fix: backend suite release gate must fail closed when suite status rows omit blockers; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_report_rejects_absolute_artifact_path() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for absolute suite artifact path test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for absolute suite artifact test.");
        let external_artifact = dir.path().join("external-suite-artifact.json");
        std::fs::write(
            &external_artifact,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
                "environment": {"host_cpu_model": "test cpu"},
                "summary": {"failed": 0, "cache_hit_rate": null},
                "cases": []
            }))
            .expect("Fix: serialize external suite artifact fixture."),
        )
        .expect("Fix: write external suite artifact fixture.");
        let suite_path = benchmark_dir.join("cuda-release-suite.json");
        std::fs::write(
            &suite_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 1,
                "artifacts": [external_artifact.display().to_string()],
                "artifact_statuses": [],
                "blockers": []
            }))
            .expect("Fix: serialize absolute suite artifact fixture."),
        )
        .expect("Fix: write absolute suite artifact fixture.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "backend suite `cuda-release-suite.json` suite artifact `"
            ) && failure.contains("must be a relative release path")),
            "Fix: release gate must reject existing absolute backend suite artifact paths before reading them; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_parity_reports_source_provenance_drift_for_matching_rows() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite parity test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for backend suite parity test.");
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-workload-01-condition-eval.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "cpu_sota_100x_contract_cases": 1,
                        "cpu_sota_100x_passing_cases": 1,
                        "source_fingerprint": "git:cuda:dirty=false",
                        "source_tree_fingerprint": "source-tree-v1:cuda"
                    }
                ],
                "artifacts": ["release/evidence/benchmarks/cuda-workload-01-condition-eval.json"]
            }))
            .expect("Fix: serialize CUDA suite for backend suite parity test."),
        )
        .expect("Fix: write CUDA suite for backend suite parity test.");
        std::fs::write(
            benchmark_dir.join("wgpu-fallback-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "cpu_sota_100x_contract_cases": 0,
                        "cpu_sota_100x_passing_cases": 0,
                        "source_fingerprint": "git:wgpu:dirty=false",
                        "source_tree_fingerprint": "source-tree-v1:wgpu"
                    }
                ],
                "artifacts": ["release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"]
            }))
            .expect("Fix: serialize WGPU suite for backend suite parity test."),
        )
        .expect("Fix: write WGPU suite for backend suite parity test.");
        let requirement = Requirement {
            id: "wgpu-fallback".to_string(),
            title: "WGPU fallback".to_string(),
            status: "required".to_string(),
            evidence: Vec::new(),
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_parity(&requirement, &release_dir, &mut failures);

        assert!(
            !failures.iter().any(|failure| failure.contains(
                "field `source_fingerprint` mismatch for family `condition-eval` case `release.condition_eval.1m`"
            )),
            "Fix: WGPU parity gate must not report raw evidence-commit fingerprint drift when source_tree_fingerprint carries the backend source identity; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "field `source_tree_fingerprint` mismatch for family `condition-eval` case `release.condition_eval.1m`: cuda=Some(\"source-tree-v1:cuda\"), wgpu=Some(\"source-tree-v1:wgpu\")"
            )),
            "Fix: WGPU parity gate must report source_tree_fingerprint drift on matching suite rows; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "field `cpu_sota_100x_contract_cases` mismatch for family `condition-eval` case `release.condition_eval.1m`: cuda=Some(1), wgpu=Some(0)"
            )),
            "Fix: WGPU parity gate must report CPU-SOTA contract proof-count drift on matching suite rows; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "field `cpu_sota_100x_passing_cases` mismatch for family `condition-eval` case `release.condition_eval.1m`: cuda=Some(1), wgpu=Some(0)"
            )),
            "Fix: WGPU parity gate must report CPU-SOTA passing proof-count drift on matching suite rows; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_parity_reports_status_blocker_drift_for_matching_rows() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite blocker parity test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir).expect(
            "Fix: create benchmark evidence directory for backend suite blocker parity test.",
        );
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "cuda",
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-workload-01-condition-eval.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "blockers": []
                    }
                ],
                "artifacts": ["release/evidence/benchmarks/cuda-workload-01-condition-eval.json"]
            }))
            .expect("Fix: serialize CUDA suite for backend suite blocker parity test."),
        )
        .expect("Fix: write CUDA suite for backend suite blocker parity test.");
        std::fs::write(
            benchmark_dir.join("wgpu-fallback-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "wgpu",
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/wgpu-workload-01-condition-eval.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "blockers": ["case `release.condition_eval.1m` failed: WGPU output drift"]
                    }
                ],
                "artifacts": ["release/evidence/benchmarks/wgpu-workload-01-condition-eval.json"]
            }))
            .expect("Fix: serialize WGPU suite for backend suite blocker parity test."),
        )
        .expect("Fix: write WGPU suite for backend suite blocker parity test.");
        let requirement = Requirement {
            id: "wgpu-fallback".to_string(),
            title: "WGPU fallback".to_string(),
            status: "required".to_string(),
            evidence: Vec::new(),
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_parity(&requirement, &release_dir, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "WGPU/CUDA suite parity blockers mismatch for family `condition-eval` case `release.condition_eval.1m`"
            ) && failure.contains("WGPU output drift")),
            "Fix: WGPU parity gate must report blocker-state drift on matching suite rows; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_parity_reports_duplicate_family_case_rows() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite duplicate parity test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir).expect(
            "Fix: create benchmark evidence directory for backend suite duplicate parity test.",
        );
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-condition-a.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m"
                    },
                    {
                        "path": "release/evidence/benchmarks/cuda-condition-b.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m"
                    }
                ],
                "artifacts": [
                    "release/evidence/benchmarks/cuda-condition-a.json",
                    "release/evidence/benchmarks/cuda-condition-b.json"
                ]
            }))
            .expect("Fix: serialize CUDA suite for backend suite duplicate parity test."),
        )
        .expect("Fix: write CUDA suite for backend suite duplicate parity test.");
        std::fs::write(
            benchmark_dir.join("wgpu-fallback-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/wgpu-condition-a.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m"
                    },
                    {
                        "path": "release/evidence/benchmarks/wgpu-condition-b.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m"
                    }
                ],
                "artifacts": [
                    "release/evidence/benchmarks/wgpu-condition-a.json",
                    "release/evidence/benchmarks/wgpu-condition-b.json"
                ]
            }))
            .expect("Fix: serialize WGPU suite for backend suite duplicate parity test."),
        )
        .expect("Fix: write WGPU suite for backend suite duplicate parity test.");
        let requirement = Requirement {
            id: "wgpu-fallback".to_string(),
            title: "WGPU fallback".to_string(),
            status: "required".to_string(),
            evidence: Vec::new(),
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_backend_suite_parity(&requirement, &release_dir, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "has 2 CUDA rows for family `condition-eval` case `release.condition_eval.1m`"
            )),
            "Fix: WGPU parity gate must report duplicate CUDA family/case rows; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "has 2 WGPU rows for family `condition-eval` case `release.condition_eval.1m`"
            )),
            "Fix: WGPU parity gate must report duplicate WGPU family/case rows; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_report_rejects_whitespace_only_status_identity_and_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite blank field test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for backend suite blank field test.");
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 1,
                "artifacts": ["release/evidence/benchmarks/cuda-workload-blank.json"],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-workload-blank.json",
                        "exists": true,
                        "bytes": 1,
                        "read_error": null,
                        "family_id": "   ",
                        "requested_case_id": "\t",
                        "source_fingerprint": "  ",
                        "source_tree_fingerprint": " ",
                        "host_cpu_model": "\n",
                        "selected_backend": "cuda",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "min_kernel_launches": 1,
                        "gpu_model": " ",
                        "nvidia_driver_version": "\t",
                        "nvidia_cuda_version": "\n",
                        "gpu_memory_total_mib": 24576,
                        "gpu_compute_capability_major": 8,
                        "gpu_compute_capability_minor": 9
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize CUDA suite with blank status fields."),
        )
        .expect("Fix: write CUDA suite with blank status fields.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        for expected in [
            "has no family_id",
            "has no requested_case_id",
            "has no `source_fingerprint` provenance",
            "has no `source_tree_fingerprint` provenance",
            "has no `host_cpu_model` provenance",
            "has no `gpu_model` provenance",
            "has no `nvidia_driver_version` provenance",
            "has no `nvidia_cuda_version` provenance",
        ] {
            assert!(
                failures.iter().any(|failure| failure.contains(expected)),
                "Fix: backend suite gate must reject whitespace-only status field `{expected}`; failures={failures:?}"
            );
        }
    }

    #[test]
    fn backend_suite_report_rejects_duplicate_family_coverage() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite duplicate family test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir).expect(
            "Fix: create benchmark evidence directory for backend suite duplicate family test.",
        );
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 2,
                "artifacts": [
                    "release/evidence/benchmarks/cuda-condition-fast.json",
                    "release/evidence/benchmarks/cuda-condition-slow.json"
                ],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/cuda-condition-fast.json",
                        "exists": true,
                        "bytes": 1,
                        "read_error": null,
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "source_fingerprint": "git:abc:dirty=false",
                        "host_cpu_model": "test CPU",
                        "selected_backend": "cuda",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "min_kernel_launches": 1,
                        "gpu_model": "RTX 5090",
                        "nvidia_driver_version": "580.0",
                        "nvidia_cuda_version": "13.0",
                        "gpu_memory_total_mib": 24576,
                        "gpu_compute_capability_major": 8,
                        "gpu_compute_capability_minor": 9
                    },
                    {
                        "path": "release/evidence/benchmarks/cuda-condition-slow.json",
                        "exists": true,
                        "bytes": 1,
                        "read_error": null,
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.10m",
                        "source_fingerprint": "git:abc:dirty=false",
                        "host_cpu_model": "test CPU",
                        "selected_backend": "cuda",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "min_kernel_launches": 1,
                        "gpu_model": "RTX 5090",
                        "nvidia_driver_version": "580.0",
                        "nvidia_cuda_version": "13.0",
                        "gpu_memory_total_mib": 24576,
                        "gpu_compute_capability_major": 8,
                        "gpu_compute_capability_minor": 9
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize CUDA suite with duplicate family coverage."),
        )
        .expect("Fix: write CUDA suite with duplicate family coverage.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "has 2 artifact_statuses rows for family `condition-eval`"
            )),
            "Fix: backend suite gate must reject repeated workload family coverage; failures={failures:?}"
        );
    }

    #[test]
    fn backend_suite_report_rejects_declared_family_count_drift() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite family count drift test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for backend suite count drift test.");
        let artifacts = (0..12)
            .map(|index| format!("release/evidence/benchmarks/cuda-workload-{index}.json"))
            .collect::<Vec<_>>();
        let artifact_statuses = artifacts
            .iter()
            .enumerate()
            .map(|(index, path)| {
                serde_json::json!({
                    "path": path,
                    "exists": true,
                    "bytes": 1,
                    "read_error": null,
                    "family_id": format!("workload-{index}"),
                    "requested_case_id": format!("release.workload_{index}.1m"),
                    "source_fingerprint": "git:abc:dirty=false",
                    "host_cpu_model": "test CPU",
                    "selected_backend": "cuda",
                    "case_count": 1,
                    "failed_count": 0,
                    "nonmatching_case_backend_count": 0,
                    "min_kernel_launches": 1,
                    "gpu_model": "RTX 5090",
                    "nvidia_driver_version": "580.0",
                    "nvidia_cuda_version": "13.0",
                    "gpu_memory_total_mib": 24576,
                    "gpu_compute_capability_major": 8,
                    "gpu_compute_capability_minor": 9
                })
            })
            .collect::<Vec<_>>();
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "cuda",
                "family_count": 13,
                "artifacts": artifacts,
                "artifact_statuses": artifact_statuses,
                "blockers": []
            }))
            .expect("Fix: serialize CUDA suite with stale family_count."),
        )
        .expect("Fix: write CUDA suite with stale family_count.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        for expected in [
            "family_count=13, but artifacts has 12 row(s)",
            "family_count=13, but artifact_statuses has 12 unique family_id row(s)",
        ] {
            assert!(
                failures.iter().any(|failure| failure.contains(expected)),
                "Fix: release gate must reject stale backend suite declared family totals; failures={failures:?}"
            );
        }
    }

    #[test]
    fn backend_suite_report_rejects_filename_backend_identity_drift() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for backend suite identity test.");
        let release_dir = dir.path().join("release");
        let benchmark_dir = release_dir.join("evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for backend suite identity test.");
        let artifact_path = benchmark_dir.join("workload-01-condition-eval.json");
        std::fs::write(
            &artifact_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                            "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002},
                            "kernel_launches": {"samples": 30, "p50": 1}
                        },
                        "performance": {"contract_passed": true, "speedup_x": 120.0}
                    }
                ]
            }))
            .expect("Fix: serialize WGPU benchmark artifact for backend suite identity test."),
        )
        .expect("Fix: write WGPU benchmark artifact for backend suite identity test.");
        std::fs::write(
            benchmark_dir.join("cuda-release-suite.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 2,
                "backend": "wgpu",
                "family_count": 1,
                "artifacts": ["release/evidence/benchmarks/workload-01-condition-eval.json"],
                "artifact_statuses": [
                    {
                        "path": "release/evidence/benchmarks/workload-01-condition-eval.json",
                        "family_id": "condition-eval",
                        "requested_case_id": "release.condition_eval.1m",
                        "exists": true,
                        "bytes": 1,
                        "read_error": null,
                        "source_fingerprint": "git:abc:dirty=false",
                        "selected_backend": "wgpu",
                        "case_count": 1,
                        "failed_count": 0,
                        "nonmatching_case_backend_count": 0,
                        "min_wall_samples": 30,
                        "min_baseline_wall_samples": 30,
                        "min_wall_p50": 10,
                        "min_wall_p95": 11,
                        "min_wall_p99": 12,
                        "min_baseline_wall_p50": 1000,
                        "min_baseline_wall_p95": 1001,
                        "min_baseline_wall_p99": 1002,
                        "min_kernel_launches": 1,
                        "blockers": []
                    }
                ],
                "blockers": []
            }))
            .expect("Fix: serialize CUDA suite with mismatched backend identity."),
        )
        .expect("Fix: write CUDA suite with mismatched backend identity.");
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["evidence/benchmarks/cuda-release-suite.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_backend_suite_report(
            &requirement,
            &release_dir,
            "cuda-release-suite.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "backend `wgpu` does not match required `cuda`"
            )),
            "Fix: CUDA suite identity must be validated against the evidence filename; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "selected backend `Some(\"wgpu\")`, expected `cuda`"
            )),
            "Fix: CUDA suite artifacts must be validated against filename-implied backend, not a bad suite backend field; failures={failures:?}"
        );
    }
}

pub(crate) fn check_backend_suite_parity(
    requirement: &Requirement,
    base_dir: &Path,
    failures: &mut Vec<String>,
) {
    let cuda = read_release_suite_json(
        requirement,
        base_dir,
        "evidence/benchmarks/cuda-release-suite.json",
        failures,
    );
    let wgpu = read_release_suite_json(
        requirement,
        base_dir,
        "evidence/benchmarks/wgpu-fallback-suite.json",
        failures,
    );
    let (Some(cuda), Some(wgpu)) = (cuda, wgpu) else {
        return;
    };
    for issue in backend_suite_parity_issues(&cuda, &wgpu) {
        match issue {
            BackendSuiteParityIssue::CudaBackendIdentity { issue } => {
                push_backend_suite_parity_backend_identity_failure(
                    requirement,
                    "CUDA",
                    issue,
                    failures,
                );
            }
            BackendSuiteParityIssue::WgpuBackendIdentity { issue } => {
                push_backend_suite_parity_backend_identity_failure(
                    requirement,
                    "WGPU",
                    issue,
                    failures,
                );
            }
            BackendSuiteParityIssue::MissingCudaPair {
                family_id,
                requested_case_id,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity has WGPU family `{family_id}` case `{requested_case_id}` with no CUDA counterpart",
                requirement.id
            )),
            BackendSuiteParityIssue::MissingWgpuPair {
                family_id,
                requested_case_id,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity has CUDA family `{family_id}` case `{requested_case_id}` with no WGPU counterpart",
                requirement.id
            )),
            BackendSuiteParityIssue::CountMismatch {
                cuda_count,
                wgpu_count,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity count mismatch: cuda={cuda_count}, wgpu={wgpu_count}",
                requirement.id
            )),
            BackendSuiteParityIssue::SharedArtifactPath { path } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity reuses artifact path `{path}` across CUDA and WGPU suites",
                requirement.id
            )),
            BackendSuiteParityIssue::DuplicateCudaPair {
                family_id,
                requested_case_id,
                count,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity has {count} CUDA rows for family `{family_id}` case `{requested_case_id}`",
                requirement.id
            )),
            BackendSuiteParityIssue::DuplicateWgpuPair {
                family_id,
                requested_case_id,
                count,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity has {count} WGPU rows for family `{family_id}` case `{requested_case_id}`",
                requirement.id
            )),
            BackendSuiteParityIssue::StatusFieldMismatch {
                family_id,
                requested_case_id,
                field,
                cuda_value,
                wgpu_value,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity field `{field}` mismatch for family `{family_id}` case `{requested_case_id}`: cuda={cuda_value:?}, wgpu={wgpu_value:?}",
                requirement.id
            )),
            BackendSuiteParityIssue::StatusStringFieldMismatch {
                family_id,
                requested_case_id,
                field,
                cuda_value,
                wgpu_value,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity field `{field}` mismatch for family `{family_id}` case `{requested_case_id}`: cuda={cuda_value:?}, wgpu={wgpu_value:?}",
                requirement.id
            )),
            BackendSuiteParityIssue::StatusBlockersMismatch {
                family_id,
                requested_case_id,
                cuda_blockers,
                wgpu_blockers,
            } => failures.push(format!(
                "requirement `{}` WGPU/CUDA suite parity blockers mismatch for family `{family_id}` case `{requested_case_id}`: cuda={cuda_blockers:?}, wgpu={wgpu_blockers:?}",
                requirement.id
            )),
        }
    }
}

fn push_backend_suite_parity_backend_identity_failure(
    requirement: &Requirement,
    suite_label: &str,
    issue: BackendSuiteBackendIssue,
    failures: &mut Vec<String>,
) {
    match issue {
        BackendSuiteBackendIssue::Missing { expected_backend } => failures.push(format!(
            "requirement `{}` WGPU/CUDA suite parity {suite_label} suite is missing backend identity `{expected_backend}`",
            requirement.id
        )),
        BackendSuiteBackendIssue::Mismatch {
            expected_backend,
            actual_backend,
        } => failures.push(format!(
            "requirement `{}` WGPU/CUDA suite parity {suite_label} suite backend `{actual_backend}` does not match required `{expected_backend}`",
            requirement.id
        )),
    }
}

fn read_release_suite_json(
    requirement: &Requirement,
    base_dir: &Path,
    evidence: &str,
    failures: &mut Vec<String>,
) -> Option<serde_json::Value> {
    let path = resolve_manifest_path(base_dir, evidence);
    let text = match read_text_bounded(&path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "requirement `{}` failed to read backend suite parity artifact `{}`: {error}",
                requirement.id,
                path.display()
            ));
            return None;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            failures.push(format!(
                "requirement `{}` backend suite parity artifact `{}` is invalid JSON: {error}",
                requirement.id,
                path.display()
            ));
            None
        }
    }
}
pub(crate) fn check_markdown_evidence_ready(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let evidence = requirement
        .evidence
        .iter()
        .find(|path| path.ends_with(suffix));
    let Some(evidence) = evidence else {
        failures.push(format!(
            "requirement `{}` needs markdown evidence ending in `{suffix}`",
            requirement.id
        ));
        return;
    };
    let path = resolve_manifest_path(base_dir, evidence);
    let text = match read_text_bounded(&path) {
        Ok(text) => text,
        Err(error) => {
            failures.push(format!(
                "requirement `{}` failed to read markdown evidence `{}`: {error}",
                requirement.id,
                path.display()
            ));
            return;
        }
    };
    for marker in [
        "status: blocked",
        "status: open",
        "status: pending",
        "todo",
        "fixme",
        "placeholder",
        "stub",
        "tbd",
        "to be filled",
    ] {
        for line in text.lines() {
            let lowered = line.to_ascii_lowercase();
            if markdown_line_is_release_rule_text(&lowered) {
                continue;
            }
            if lowered.contains(marker) {
                failures.push(format!(
                    "requirement `{}` markdown evidence `{}` contains unresolved marker `{marker}`",
                    requirement.id,
                    path.display()
                ));
                break;
            }
        }
    }
    if text.trim().is_empty() {
        failures.push(format!(
            "requirement `{}` markdown evidence `{}` is empty",
            requirement.id,
            path.display()
        ));
    }
    if !text.contains("Evidence sources:") {
        failures.push(format!(
            "requirement `{}` markdown evidence `{}` does not list evidence sources",
            requirement.id,
            path.display()
        ));
    }
}

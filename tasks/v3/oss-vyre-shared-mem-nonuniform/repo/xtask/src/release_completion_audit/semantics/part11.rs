use crate::benchmark_evidence_semantics::{
    backend_consistency_issues, benchmark_report_has_source_provenance,
    benchmark_source_artifact_paths, cuda_forbidden_telemetry_issues,
    cuda_telemetry_label_issues, launch_plan_label_issues, BackendConsistencyIssue,
    CudaForbiddenTelemetryIssue, CudaTelemetryLabelIssue, LaunchPlanLabelIssue,
};

fn inspect_workload_benchmark_semantics(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("selected_backend")
        .and_then(serde_json::Value::as_str)
        != Some("cuda")
    {
        blockers.push(format!("{evidence}: selected_backend must be cuda"));
    }
    inspect_workload_benchmark_provenance(evidence, path, value, blockers);
    let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing cases array"));
        return;
    };
    if cases.is_empty() {
        blockers.push(format!("{evidence}: cases array is empty"));
        return;
    }
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if case.get("backend_id").and_then(serde_json::Value::as_str) != Some("cuda") {
            blockers.push(format!("{evidence}: case `{id}` backend_id must be cuda"));
        }
        if case.get("contract").is_none_or(serde_json::Value::is_null) {
            blockers.push(format!("{evidence}: case `{id}` is missing a contract"));
        }
        if !case
            .get("performance")
            .and_then(|performance| performance.get("contract_passed"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            let reason = crate::benchmark_evidence_semantics::benchmark_case_failure_reason(case)
                .map(|reason| format!(": {reason}"))
                .unwrap_or_default();
            blockers.push(format!(
                "{evidence}: case `{id}` must pass its performance contract{reason}"
            ));
        }
        let metrics = case.get("metrics").and_then(serde_json::Value::as_object);
        let wall = metrics.and_then(active_gpu_metric_p50);
        let baseline = metrics.and_then(|metrics| metric_p50(metrics.get("baseline_wall_ns")));
        let wall_samples = metrics
            .and_then(|metrics| metric_samples(metrics.get("wall_ns")))
            .unwrap_or(0);
        if wall_samples < 30 {
            blockers.push(format!(
                "{evidence}: case `{id}` has {wall_samples} wall_ns sample(s), needs at least 30"
            ));
        }
        let baseline_wall_samples = metrics
            .and_then(|metrics| metric_samples(metrics.get("baseline_wall_ns")))
            .unwrap_or(0);
        if baseline_wall_samples < 30 {
            blockers.push(format!(
                "{evidence}: case `{id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30"
            ));
        }
        require_benchmark_metric_percentiles(evidence, id, metrics, "wall_ns", blockers);
        require_benchmark_metric_percentiles(evidence, id, metrics, "baseline_wall_ns", blockers);
        match (wall, baseline) {
            (Some(wall), Some(baseline)) if wall > 0.0 && baseline > wall => {}
            (Some(wall), Some(baseline)) => blockers.push(format!(
                "{evidence}: case `{id}` did not beat p50 CPU/SOTA baseline: wall={wall:.2}, baseline={baseline:.2}"
            )),
            _ => blockers.push(format!(
                "{evidence}: case `{id}` must include p50 wall_ns and baseline_wall_ns"
            )),
        }
        let speedup = case
            .get("performance")
            .and_then(|performance| performance.get("speedup_x"))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        if speedup <= 1.0 {
            blockers.push(format!(
                "{evidence}: case `{id}` speedup_x must be greater than 1.0"
            ));
        }
    }
}

fn inspect_workload_benchmark_provenance(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    inspect_benchmark_report_provenance(evidence, path, value, blockers);
    check_case_backend_matches_selected_backend(evidence, value, blockers);
    inspect_contract_baselines_apply_to_backend(evidence, value, blockers);
    check_cuda_telemetry_labels_match_counters(evidence, value, blockers);
    check_cuda_forbidden_telemetry_is_zero(evidence, value, blockers);
    let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
        return;
    };
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if !has_nonempty_string_any(
            case,
            &[
                "dataset_fingerprint",
                "corpus_fingerprint",
                "input_fingerprint",
                "workload_fingerprint",
            ],
        ) && !case.get("contract").is_some_and(|contract| {
            has_nonempty_string_any(
                contract,
                &[
                    "dataset_fingerprint",
                    "corpus_fingerprint",
                    "input_fingerprint",
                    "workload_fingerprint",
                ],
            )
        }) {
            blockers.push(format!(
                "{evidence}: case `{id}` must include dataset/corpus/input fingerprint provenance"
            ));
        }
        if !case
            .get("correctness")
            .is_some_and(|correctness| !correctness.is_null())
            && !case.get("oracle").is_some_and(|oracle| !oracle.is_null())
        {
            blockers.push(format!(
                "{evidence}: case `{id}` must include correctness oracle evidence"
            ));
        }
        let metrics = case.get("metrics").and_then(serde_json::Value::as_object);
        for (label, metric_names) in [
            (
                "cold compile or cold wall timing",
                &["cold_compile_ns", "cold_wall_ns", "compile_ns"][..],
            ),
            (
                "host-to-device transfer bytes",
                &[
                    "host_to_device_bytes",
                    "h2d_bytes",
                    "bytes_host_to_device",
                    "bytes_h2d",
                ][..],
            ),
            (
                "device-to-host transfer bytes",
                &[
                    "device_to_host_bytes",
                    "d2h_bytes",
                    "bytes_device_to_host",
                    "bytes_d2h",
                ][..],
            ),
        ] {
            if !metrics_has_any(metrics, metric_names) {
                blockers.push(format!(
                    "{evidence}: case `{id}` must include {label} metric"
                ));
            }
        }
        if is_non_dispatch_proof_case(id) {
            if !metrics_has_zero_any(metrics, LAUNCH_COUNT_METRICS) {
                blockers.push(format!(
                    "{evidence}: case `{id}` must include zero kernel launch count metric for non-dispatch proof evidence"
                ));
            }
        } else if !metrics_has_positive_any(metrics, LAUNCH_COUNT_METRICS) {
            blockers.push(format!(
                "{evidence}: case `{id}` must include positive kernel launch count metric"
            ));
        }
        check_launch_plan_label_matches_count(evidence, id, case, metrics, blockers);
        if !case
            .get("optimization_passes")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| !items.is_empty())
            && !case
                .get("optimization_passes_applied")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| !items.is_empty())
        {
            blockers.push(format!(
                "{evidence}: case `{id}` must list optimization passes applied"
            ));
        }
    }
}

fn inspect_benchmark_report_provenance(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if !benchmark_report_has_source_provenance(value) {
        blockers.push(format!(
            "{evidence}: benchmark report must include source_fingerprint provenance"
        ));
    }
    if let Some(source_fingerprint) = value
        .get("source_fingerprint")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        inspect_source_fingerprint_shape(evidence, source_fingerprint, blockers);
    }
    let source_artifacts = benchmark_source_artifact_paths(value);
    match release_evidence_workspace_root(path) {
        Some(workspace_root) => {
            for artifact in source_artifacts {
                if let Some(issue) =
                    crate::benchmark_evidence_semantics::benchmark_source_artifact_path_issue(
                        workspace_root,
                        &artifact,
                    )
                {
                    blockers.push(format!(
                        "{evidence}: {}",
                        issue.describe("source_artifact", &artifact)
                    ));
                }
            }
        }
        None if !source_artifacts.is_empty() => blockers.push(format!(
            "{evidence}: could not resolve workspace root for source_artifacts from {}",
            path.display()
        )),
        None => {}
    }
    let environment = value.get("environment");
    if !environment.is_some_and(|environment| {
        has_nonempty_string_any(
            environment,
            &["host_cpu_model", "cpu_model", "host_cpu", "processor_model"],
        )
    }) {
        blockers.push(format!(
            "{evidence}: benchmark environment must include host CPU model provenance"
        ));
    }
    let summary = value.get("summary");
    if !summary.is_some_and(|summary| summary.get("cache_hit_rate").is_some()) {
        blockers.push(format!(
            "{evidence}: benchmark summary must include cache_hit_rate, even when null"
        ));
    }
}

fn has_nonempty_string_any(value: &serde_json::Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        value
            .get(*field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    })
}

fn metrics_has_any(
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    fields: &[&str],
) -> bool {
    metrics.is_some_and(|metrics| {
        fields.iter().any(|field| {
            metrics.get(*field).is_some_and(|value| {
                metric_samples(Some(value)).is_some_and(|samples| samples > 0)
                    || metric_p50(Some(value)).is_some_and(|sample| sample > 0.0)
                    || value.as_u64().is_some()
                    || value.as_f64().is_some_and(|number| number >= 0.0)
            })
        })
    })
}

fn metrics_has_positive_any(
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    fields: &[&str],
) -> bool {
    metrics.is_some_and(|metrics| {
        fields.iter().any(|field| {
            metrics.get(*field).is_some_and(|value| {
                metric_p50(Some(value)).is_some_and(|sample| sample > 0.0)
                    || value.as_u64().is_some_and(|number| number > 0)
                    || value.as_f64().is_some_and(|number| number > 0.0)
            })
        })
    })
}

const LAUNCH_COUNT_METRICS: &[&str] = &["kernel_launches", "launch_count", "launches"];
const NON_DISPATCH_PROOF_CASE_IDS: &[&str] = &[
    "cuda.ptx.patterns.release.corpus",
    "lower.rewrites.impact.corpus",
    "lower.egraph_saturation",
    "lower.alias_aware_optimizations",
];

fn is_non_dispatch_proof_case(case_id: &str) -> bool {
    NON_DISPATCH_PROOF_CASE_IDS.contains(&case_id)
}

fn metrics_has_zero_any(
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    fields: &[&str],
) -> bool {
    metrics.is_some_and(|metrics| {
        fields.iter().any(|field| {
            metrics.get(*field).is_some_and(|value| {
                metric_p50(Some(value)).is_some_and(|sample| sample == 0.0)
                    || value.as_u64() == Some(0)
                    || value.as_f64().is_some_and(|number| number == 0.0)
            })
        })
    })
}

fn check_launch_plan_label_matches_count(
    evidence: &str,
    case_id: &str,
    case: &serde_json::Value,
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    blockers: &mut Vec<String>,
) {
    for issue in launch_plan_label_issues(case, metrics) {
        match issue {
            LaunchPlanLabelIssue::MissingSingle => blockers.push(format!(
                "{evidence}: case `{case_id}` reports one kernel launch but is missing `single-dispatch-launch-plan`"
            )),
            LaunchPlanLabelIssue::SingleHasMulti => blockers.push(format!(
                "{evidence}: case `{case_id}` reports one kernel launch but lists `multi-dispatch-launch-plan`"
            )),
            LaunchPlanLabelIssue::MissingMulti { launch_count } => blockers.push(format!(
                "{evidence}: case `{case_id}` reports {launch_count:.0} kernel launches but is missing `multi-dispatch-launch-plan`"
            )),
            LaunchPlanLabelIssue::MultiHasSingle { launch_count } => blockers.push(format!(
                "{evidence}: case `{case_id}` reports {launch_count:.0} kernel launches but lists `single-dispatch-launch-plan`"
            )),
        }
    }
}

fn inspect_contract_baselines_apply_to_backend(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    for issue in crate::benchmark_evidence_semantics::contract_backend_issues(value) {
        match issue {
            crate::benchmark_evidence_semantics::ContractBackendIssue::MissingBaselines {
                case_id,
                backend_id,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` backend `{backend_id}` has a performance contract with no baselines"
            )),
            crate::benchmark_evidence_semantics::ContractBackendIssue::NoApplicableBaseline {
                case_id,
                backend_id,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` backend `{backend_id}` has no applicable performance contract baseline"
            )),
        }
    }
}

fn inspect_source_fingerprint_shape(
    evidence: &str,
    source_fingerprint: &str,
    blockers: &mut Vec<String>,
) {
    for issue in crate::benchmark_evidence_semantics::source_fingerprint_issues(source_fingerprint)
    {
        match issue {
            crate::benchmark_evidence_semantics::SourceFingerprintIssue::DirtyUnknownState {
                source_fingerprint,
            } => blockers.push(format!(
                "{evidence}: source_fingerprint `{source_fingerprint}` has dirty=unknown; rerun with git status provenance available"
            )),
            crate::benchmark_evidence_semantics::SourceFingerprintIssue::DirtyMissingWorktree {
                source_fingerprint,
            } => blockers.push(format!(
                "{evidence}: source_fingerprint `{source_fingerprint}` is dirty but has no worktree digest"
            )),
            crate::benchmark_evidence_semantics::SourceFingerprintIssue::DirtyUnknownWorktree {
                source_fingerprint,
            } => blockers.push(format!(
                "{evidence}: source_fingerprint `{source_fingerprint}` has an unknown worktree digest"
            )),
            crate::benchmark_evidence_semantics::SourceFingerprintIssue::DirtyInvalidWorktree {
                source_fingerprint,
                worktree,
            } => blockers.push(format!(
                "{evidence}: source_fingerprint `{source_fingerprint}` has invalid worktree digest `{worktree}`; expected 64 hex chars"
            )),
        }
    }
}

fn check_case_backend_matches_selected_backend(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    for issue in backend_consistency_issues(value) {
        match issue {
            BackendConsistencyIssue::MissingCaseId { case_index } => blockers.push(format!(
                "{evidence}: case index {case_index} must include a nonblank id"
            )),
            BackendConsistencyIssue::DuplicateCaseId { case_id, count } => blockers.push(format!(
                "{evidence}: has {count} cases with id `{case_id}`"
            )),
            BackendConsistencyIssue::MissingCaseBackend {
                case_id,
                expected_backend,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` must include backend_id `{expected_backend}` matching selected_backend"
            )),
            BackendConsistencyIssue::CaseBackendMismatch {
                case_id,
                expected_backend,
                actual_backend,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` backend_id `{actual_backend}` does not match selected_backend `{expected_backend}`"
            )),
        }
    }
}

fn check_cuda_telemetry_labels_match_counters(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    for issue in cuda_telemetry_label_issues(value) {
        match issue {
            CudaTelemetryLabelIssue::MissingLabel {
                case_id,
                label: telemetry_label,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` has positive CUDA telemetry counters but is missing `{telemetry_label}`"
            )),
            CudaTelemetryLabelIssue::LabelWithoutCounters {
                case_id,
                label: telemetry_label,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` lists `{telemetry_label}` but all matching CUDA telemetry counters are zero or missing"
            )),
        }
    }
}

fn check_cuda_forbidden_telemetry_is_zero(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    for issue in cuda_forbidden_telemetry_issues(value) {
        match issue {
            CudaForbiddenTelemetryIssue::ResidentBorrowedEscapeHatch {
                case_id,
                observed_p50,
            } => blockers.push(format!(
                "{evidence}: case `{case_id}` has cuda_resident_borrowed_fallback_dispatches p50={observed_p50}; release CUDA benchmark evidence must use native resident dispatch"
            )),
        }
    }
}

fn inspect_weir_readme_contract_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    inspect_schema_version_at_least(evidence, value, 2, blockers);
    if value.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
        blockers.push(format!("{evidence}: README.md must exist"));
    }
    if value
        .get("source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        blockers.push(format!("{evidence}: README.md is empty"));
    }
    if value
        .get("missing_tokens")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|tokens| !tokens.is_empty())
    {
        blockers.push(format!(
            "{evidence}: missing_tokens must exist and be empty"
        ));
    }
    if value
        .get("example_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        blockers.push(format!(
            "{evidence}: README.md must contain at least one Rust or TOML example"
        ));
    }
    if value
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|items| !items.is_empty())
    {
        blockers.push(format!("{evidence}: blockers must exist and be empty"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_audit_launch_metric_rejects_zero_sampled_counts() {
        let metrics = serde_json::json!({
            "kernel_launches": {
                "p50": 0,
                "samples": 30
            }
        });
        let metrics = metrics.as_object();

        assert!(
            metrics_has_any(metrics, &["kernel_launches"]),
            "Fix: this fixture must still demonstrate the old presence-only weakness."
        );
        assert!(
            !metrics_has_positive_any(metrics, &["kernel_launches", "launch_count", "launches"]),
            "Fix: completion audit launch evidence must reject zero-valued launch metrics even when samples are present."
        );
    }

    #[test]
    fn completion_audit_launch_metric_accepts_positive_aliases() {
        let percentile = serde_json::json!({
            "launches": {
                "p50": 2,
                "samples": 30
            }
        });
        assert!(metrics_has_positive_any(
            percentile.as_object(),
            &["kernel_launches", "launch_count", "launches"]
        ));

        let scalar = serde_json::json!({
            "kernel_launches": 1
        });
        assert!(metrics_has_positive_any(
            scalar.as_object(),
            &["kernel_launches", "launch_count", "launches"]
        ));
    }

    #[test]
    fn completion_audit_non_dispatch_proof_launch_metric_requires_zero_value() {
        let metrics = serde_json::json!({
            "kernel_launches": {
                "p50": 0,
                "samples": 30
            }
        });
        let metrics = metrics.as_object();

        for case_id in NON_DISPATCH_PROOF_CASE_IDS {
            assert!(
                is_non_dispatch_proof_case(case_id),
                "Fix: `{case_id}` must be recognized as a zero-dispatch proof case."
            );
        }
        assert!(metrics_has_zero_any(metrics, LAUNCH_COUNT_METRICS));
        assert!(
            !metrics_has_positive_any(metrics, LAUNCH_COUNT_METRICS),
            "Fix: non-dispatch proof evidence must prove zero dispatches instead of being counted as a launched CUDA kernel."
        );
    }

    #[test]
    fn completion_audit_workload_failure_preserves_case_reason() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "git": {"commit": "0123456789abcdef0123456789abcdef01234567"},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "failed",
                    "correctness": {
                        "Invalid": {
                            "reason": "Performance contract failed: release condition eval requires 100.00x over CPU-SOTA, observed 42.00x"
                        }
                    },
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": null,
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002}
                    }
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_workload_benchmark_semantics(
            "workload-failed.json",
            Path::new("workload-failed.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "case `release.condition_eval.1m` must pass its performance contract: Performance contract failed"
            ) && blocker.contains("observed 42.00x")),
            "Fix: completion audit workload blockers must preserve failed benchmark case reasons; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_cuda_borrowed_escape_hatch_telemetry() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
            "environment": {"host_cpu_model": "test cpu"},
            "summary": {"cache_hit_rate": 0.0, "failed": 0},
            "cases": [
                {
                    "id": "release.callgraph_reachability.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "correctness": {"Valid": true},
                    "contract": {
                        "dataset_fingerprint": "sha256:callgraph",
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0},
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2100, "p99": 2200},
                        "kernel_launches": {"samples": 30, "p50": 1},
                        "cuda_resident_borrowed_fallback_dispatches": {"samples": 30, "p50": 3}
                    },
                    "optimization_passes_applied": [
                        "cuda-explicit-backend-selection",
                        "single-dispatch-launch-plan",
                        "cuda-resident-borrowed-escape-hatch"
                    ]
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_workload_benchmark_semantics(
            "release/evidence/benchmarks/workload-13-callgraph-reachability.json",
            Path::new("release/evidence/benchmarks/workload-13-callgraph-reachability.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "case `release.callgraph_reachability.1m` has cuda_resident_borrowed_fallback_dispatches p50=3"
            ) && blocker.contains("must use native resident dispatch")),
            "Fix: completion audit must reject CUDA benchmark evidence collected through the resident borrowed escape hatch; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_json_summary_preserves_failed_case_reason() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for completion audit JSON test.");
        let path = dir.path().join("wgpu-workload-failed.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "summary": {"failed": 1},
                "cases": [
                    {
                        "id": "sparse.compaction.count.1m",
                        "status": "failed",
                        "correctness": {
                            "Invalid": {
                                "reason": "Performance contract failed: sparse output compaction count requires 100.00x over CPU-SOTA, observed 86.90x"
                            }
                        }
                    }
                ]
            }))
            .expect("Fix: serialize failed completion audit JSON fixture."),
        )
        .expect("Fix: write failed completion audit JSON fixture.");
        let mut blockers = Vec::new();

        inspect_json_evidence("wgpu-workload-failed.json", &path, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "benchmark summary reports 1 failed case(s): `sparse.compaction.count.1m`: Performance contract failed"
            ) && blocker.contains("observed 86.90x")),
            "Fix: completion audit JSON summary blockers must preserve failed benchmark case reasons; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_generic_benchmark_missing_source_artifact_file() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for benchmark source artifact audit test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let evidence_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let path = evidence_dir.join("wgpu-missing-source-artifact.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "source_artifacts": ["release/evidence/benchmarks/missing-source.json"],
                "environment": {"host_cpu_model": "test cpu"},
                "summary": {"failed": 0, "cache_hit_rate": null},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize benchmark source artifact audit fixture."),
        )
        .expect("Fix: write benchmark source artifact audit fixture.");
        let mut blockers = Vec::new();

        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-missing-source-artifact.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "source_artifact `release/evidence/benchmarks/missing-source.json` is not a readable file"
            )),
            "Fix: completion audit must reject benchmark source_artifacts that do not resolve to files; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_absolute_benchmark_source_artifact_file() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for absolute source artifact audit test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let evidence_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let external_source = dir.path().join("external-source.json");
        std::fs::write(&external_source, "{}").expect("Fix: write external source artifact.");
        let path = evidence_dir.join("wgpu-absolute-source-artifact.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567:dirty=false",
                "source_artifacts": [external_source.display().to_string()],
                "environment": {"host_cpu_model": "test cpu"},
                "summary": {"failed": 0, "cache_hit_rate": null},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize absolute source artifact audit fixture."),
        )
        .expect("Fix: write absolute source artifact audit fixture.");
        let mut blockers = Vec::new();

        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-absolute-source-artifact.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/wgpu-absolute-source-artifact.json: source_artifact `"
            ) && blocker.contains("must be a relative release path")),
            "Fix: completion audit must reject existing absolute benchmark source_artifact files; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_blank_benchmark_case_identity() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567;dirty=false",
            "environment": {"host_cpu_model": "test cpu"},
            "summary": {"cache_hit_rate": null},
            "cases": [
                {
                    "id": " \t ",
                    "backend_id": "cuda",
                    "status": "pass"
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_workload_benchmark_provenance(
            "cuda-blank-case-id.json",
            Path::new("cuda-blank-case-id.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker
                == "cuda-blank-case-id.json: case index 0 must include a nonblank id"),
            "Fix: completion audit must reject benchmark cases without stable nonblank identity; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_benchmark_case_identity() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "source_fingerprint": "git:0123456789abcdef0123456789abcdef01234567;dirty=false",
            "environment": {"host_cpu_model": "test cpu"},
            "summary": {"cache_hit_rate": null},
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass"
                },
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass"
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_workload_benchmark_provenance(
            "cuda-duplicate-case-id.json",
            Path::new("cuda-duplicate-case-id.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker
                == "cuda-duplicate-case-id.json: has 2 cases with id `release.condition_eval.1m`"),
            "Fix: completion audit must reject duplicate benchmark case ids before case counts can prove release coverage; blockers={blockers:?}"
        );
    }
}

fn inspect_schema_version_at_least(
    evidence: &str,
    value: &serde_json::Value,
    minimum: u64,
    blockers: &mut Vec<String>,
) {
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < minimum {
        blockers.push(format!(
            "{evidence}: schema_version is {schema_version}, expected >= {minimum}"
        ));
    }
}

fn inspect_c_parser_corpus_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let total = value
        .get("total_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let parsed = value
        .get("parsed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let failed = value
        .get("failed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let source_bytes = value
        .get("total_source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let ast_bytes = value
        .get("total_ast_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let vast_bytes = value
        .get("total_vast_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let semantic_graph_bytes = value
        .get("total_semantic_graph_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if total < 250 {
        blockers.push(format!(
            "{evidence}: total_files {total} is below Linux subsystem floor 250"
        ));
    }
    if parsed != total || failed != 0 {
        blockers.push(format!(
            "{evidence}: parsed_files={parsed}, total_files={total}, failed_files={failed}; full corpus parse required"
        ));
    }
    if source_bytes < 4 * 1024 * 1024 {
        blockers.push(format!(
            "{evidence}: total_source_bytes {source_bytes} is below Linux subsystem floor 4194304"
        ));
    }
    if value
        .get("linux_subsystem_candidate")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!(
            "{evidence}: linux_subsystem_candidate must be true"
        ));
    }
    if value
        .get("corpus_root_canonical")
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        blockers.push(format!("{evidence}: missing corpus_root_canonical"));
    }
    inspect_corpus_fingerprint(evidence, value, blockers);
    inspect_linux_subsystem_provenance(evidence, value, blockers);
    inspect_c_parser_collection_provenance(evidence, value, blockers);
    for field in ["include_dirs", "macros"] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            blockers.push(format!(
                "{evidence}: reproducibility field `{field}` must be non-empty"
            ));
        }
    }
    if ast_bytes == 0 || vast_bytes == 0 || semantic_graph_bytes == 0 {
        blockers.push(format!(
            "{evidence}: AST/VAST/semantic section bytes are incomplete: ast={ast_bytes}, vast={vast_bytes}, semantic={semantic_graph_bytes}"
        ));
    }
    let file_entries = value
        .get("files")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len) as u64;
    if file_entries != parsed {
        blockers.push(format!(
            "{evidence}: files array has {file_entries} entries, parsed_files is {parsed}"
        ));
    }
    let failure_entries = value
        .get("failures")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len) as u64;
    if failure_entries != failed {
        blockers.push(format!(
            "{evidence}: failures array has {failure_entries} entries, failed_files is {failed}"
        ));
    }
}

pub(crate) fn check_backend_feature_markers(
    requirement_id: &str,
    matrix: &serde_json::Value,
    field: &str,
    minimum: usize,
    failures: &mut Vec<String>,
) {
    let Some(markers) = matrix.get(field).and_then(serde_json::Value::as_array) else {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix is missing `{field}`"
        ));
        return;
    };
    if markers.len() < minimum {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix `{field}` has {} marker(s), needs at least {minimum}",
            markers.len()
        ));
    }
    let required_ids: &[&str] = match field {
        "cuda_feature_markers" => &[
            "tensor-core-fragment",
            "ldmatrix-cp-async",
            "predicated-execution",
            "instruction-scheduling",
            "ptx-vector-load-gap-scheduling",
            "ptx-compute-load-gap-scheduling",
            "ptx-vector-load-fusion",
            "ptx-vector-store-fusion",
            "async-copy-emitter",
            "mma-emitter",
            "cuda-resident-dispatch",
            "cuda-resident-io",
            "cuda-graph-launch",
            "cuda-module-cache",
            "cuda-ptx-source-cache",
            "cuda-ptx-target-probe",
            "megakernel-paired-speculation",
        ],
        "wgpu_feature_markers" => &[
            "wgpu-persistent-engine",
            "wgpu-megakernel-dispatcher",
            "wgpu-readback-ring",
            "wgpu-async-dispatch-prefetch",
            "wgpu-dispatch-scratch-reuse",
            "wgpu-disk-cache",
            "wgpu-no-cpu-fallback-test",
            "megakernel-paired-speculation",
        ],
        _ => &[],
    };
    for required_id in required_ids {
        if !markers.iter().any(|marker| {
            marker.get("id").and_then(serde_json::Value::as_str) == Some(*required_id)
        }) {
            failures.push(format!(
                "requirement `{requirement_id}` backend matrix `{field}` is missing required marker `{required_id}`"
            ));
        }
    }
    for marker in markers {
        let id = marker
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if marker.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
            failures.push(format!(
                "requirement `{requirement_id}` backend marker `{id}` in `{field}` does not exist"
            ));
        }
        if marker
            .get("source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(format!(
                "requirement `{requirement_id}` backend marker `{id}` in `{field}` is empty"
            ));
        }
        if marker
            .get("missing_tokens")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|tokens| !tokens.is_empty())
        {
            failures.push(format!(
                "requirement `{requirement_id}` backend marker `{id}` in `{field}` has missing implementation tokens"
            ));
        }
        if marker
            .get("unresolved_markers")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|markers| !markers.is_empty())
        {
            failures.push(format!(
                "requirement `{requirement_id}` backend marker `{id}` in `{field}` has unresolved markers"
            ));
        }
    }
}
pub(crate) fn check_readme_contract(
    requirement_id: &str,
    product: &str,
    value: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    if value.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
        failures.push(format!(
            "requirement `{requirement_id}` {product} README contract does not prove README.md exists"
        ));
    }
    if value
        .get("source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        failures.push(format!(
            "requirement `{requirement_id}` {product} README contract reports empty README.md"
        ));
    }
    if value
        .get("missing_tokens")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|tokens| !tokens.is_empty())
    {
        failures.push(format!(
            "requirement `{requirement_id}` {product} README is missing required API/version tokens"
        ));
    }
    if value
        .get("example_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        == 0
    {
        failures.push(format!(
            "requirement `{requirement_id}` {product} README has no example block"
        ));
    }
    let blockers = value
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if blockers != 0 {
        failures.push(format!(
            "requirement `{requirement_id}` {product} README contract reports {blockers} blocker(s)"
        ));
    }
}
pub(crate) const REQUIRED_BENCHMARKED_OPTIMIZATION_FAMILIES: &[&str] = &[
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
pub(crate) fn check_before_after_benchmark_report(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    check_json_value_has_no_blockers(
        requirement,
        &format!("benchmark `{suffix}`"),
        &report,
        failures,
    );
    let failed = report
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let failed_cases =
        crate::benchmark_evidence_semantics::benchmark_failed_case_summaries(&report);
    let case_failed = failed_cases.len() as u64;
    if let Some(mismatch) =
        crate::benchmark_evidence_semantics::benchmark_report_summary_case_evidence_mismatch(
            &report,
        )
    {
        failures.push(format!(
            "requirement `{}` benchmark `{suffix}` has invalid summary: {mismatch}",
            requirement.id
        ));
    }
    if failed != 0 || case_failed != 0 {
        let detail = if failed_cases.is_empty() {
            String::new()
        } else {
            format!(": {}", failed_cases.join("; "))
        };
        let count_detail = if failed == case_failed {
            String::new()
        } else {
            format!("; case evidence reports {case_failed} failed case(s)")
        };
        failures.push(format!(
            "requirement `{}` benchmark `{suffix}` reports {failed} failed case(s){count_detail}{detail}",
            requirement.id
        ));
    }
    let selected_backend = report
        .get("selected_backend")
        .and_then(serde_json::Value::as_str);
    if selected_backend.is_some() && selected_backend != Some("cuda") {
        failures.push(format!(
            "requirement `{}` benchmark `{suffix}` selected backend `{:?}`, expected cuda",
            requirement.id, selected_backend
        ));
    }
    check_benchmark_reproducibility_provenance(requirement, suffix, base_dir, &report, failures);
    let Some(cases) = report.get("cases").and_then(serde_json::Value::as_array) else {
        failures.push(format!(
            "requirement `{}` benchmark `{suffix}` has no cases array",
            requirement.id
        ));
        return;
    };
    if cases.is_empty() {
        failures.push(format!(
            "requirement `{}` benchmark `{suffix}` has zero cases",
            requirement.id
        ));
    }
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        let metrics = case.get("metrics").and_then(serde_json::Value::as_object);
        let has_wall = metrics.is_some_and(|metrics| metrics.contains_key("wall_ns"));
        let has_baseline = metrics.is_some_and(|metrics| metrics.contains_key("baseline_wall_ns"));
        if !has_wall || !has_baseline {
            failures.push(format!(
                "requirement `{}` benchmark `{suffix}` case `{id}` must contain wall_ns and baseline_wall_ns metrics",
                requirement.id
            ));
        }
        let wall_samples = metrics
            .and_then(|metrics| metric_samples(metrics.get("wall_ns")))
            .unwrap_or(0);
        if wall_samples < 30 {
            failures.push(format!(
                "requirement `{}` benchmark `{suffix}` case `{id}` has {wall_samples} wall_ns sample(s), needs at least 30",
                requirement.id
            ));
        }
        let baseline_wall_samples = metrics
            .and_then(|metrics| metric_samples(metrics.get("baseline_wall_ns")))
            .unwrap_or(0);
        if baseline_wall_samples < 30 {
            failures.push(format!(
                "requirement `{}` benchmark `{suffix}` case `{id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30",
                requirement.id
            ));
        }
        require_benchmark_metric_percentiles(
            &requirement.id,
            suffix,
            id,
            metrics,
            "wall_ns",
            failures,
        );
        require_benchmark_metric_percentiles(
            &requirement.id,
            suffix,
            id,
            metrics,
            "baseline_wall_ns",
            failures,
        );
        if let Some(metrics) = metrics {
            let wall_p50 = active_gpu_metric_p50(metrics);
            let baseline_p50 = metric_p50(metrics.get("baseline_wall_ns"));
            let egraph_quality_win = suffix == "egraph-before-after.json"
                && metric_p50(metrics.get("egraph_output_ops"))
                    .zip(metric_p50(metrics.get("egraph_baseline_ops_after")))
                    .is_some_and(|(output, baseline)| output < baseline)
                && metric_p50(metrics.get("egraph_applied_rewrites"))
                    .is_some_and(|rewrites| rewrites > 0.0);
            match (wall_p50, baseline_p50) {
                (Some(wall), Some(baseline)) if wall < baseline => {}
                (Some(_), Some(_)) if egraph_quality_win => {}
                (Some(_), Some(_)) if before_after_semantic_win(id, metrics) => {}
                (Some(wall), Some(baseline)) => failures.push(format!(
                    "requirement `{}` benchmark `{suffix}` case `{id}` did not improve p50 wall time: wall={wall:.2}, baseline={baseline:.2}",
                    requirement.id
                )),
                _ => failures.push(format!(
                    "requirement `{}` benchmark `{suffix}` case `{id}` must contain p50 values for wall_ns and baseline_wall_ns",
                    requirement.id
                )),
            }
        }
    }
}
pub(crate) fn metric_p50(metric: Option<&serde_json::Value>) -> Option<f64> {
    let metric = metric?;
    metric_percentile(Some(metric), "p50")
        .or_else(|| metric.as_f64())
        .or_else(|| metric.as_u64().map(|value| value as f64))
}
pub(crate) fn active_gpu_metric_p50(
    metrics: &serde_json::Map<String, serde_json::Value>,
) -> Option<f64> {
    metric_p50(metrics.get("dispatch_ns"))
        .or_else(|| metric_p50(metrics.get("kernel_execute_ns")))
        .or_else(|| metric_p50(metrics.get("wall_ns")))
}
pub(crate) fn before_after_semantic_win(
    case_id: &str,
    metrics: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    match case_id {
        "lower.rewrites.impact.corpus" => {
            metric_p50(metrics.get("lower_ops_eliminated")).is_some_and(|value| value > 0.0)
                || metric_p50(metrics.get("lower_optimized_issue_score"))
                    .zip(metric_p50(metrics.get("lower_baseline_issue_score")))
                    .is_some_and(|(optimized, baseline)| optimized < baseline)
        }
        "foundation.optimizer.impact" => {
            metric_p50(metrics.get("optimizer_nodes_eliminated")).is_some_and(|value| value > 0.0)
        }
        "lower.egraph_saturation" => {
            metric_p50(metrics.get("egraph_applied_rewrites")).is_some_and(|value| value > 0.0)
                && metric_p50(metrics.get("egraph_output_ops"))
                    .zip(metric_p50(metrics.get("egraph_baseline_ops_after")))
                    .is_some_and(|(output, baseline)| output < baseline)
        }
        "lower.alias_aware_optimizations" => {
            metric_p50(metrics.get("alias_pass_wins")).is_some_and(|value| value >= 5.0)
        }
        _ => false,
    }
}
pub(crate) fn metric_percentile(
    metric: Option<&serde_json::Value>,
    percentile: &str,
) -> Option<f64> {
    let metric = metric?;
    metric
        .get(percentile)
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            metric
                .get(percentile)
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as f64)
        })
}
pub(crate) fn metric_samples(metric: Option<&serde_json::Value>) -> Option<u64> {
    metric?.get("samples").and_then(serde_json::Value::as_u64)
}
pub(crate) fn require_benchmark_metric_percentiles(
    requirement_id: &str,
    benchmark: &str,
    case_id: &str,
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    metric_name: &str,
    failures: &mut Vec<String>,
) {
    for percentile in ["p50", "p95", "p99"] {
        let value =
            metrics.and_then(|metrics| metric_percentile(metrics.get(metric_name), percentile));
        if !value.is_some_and(|value| value > 0.0) {
            failures.push(format!(
                "requirement `{requirement_id}` benchmark `{benchmark}` case `{case_id}` must include positive {percentile} {metric_name}"
            ));
        }
    }
}
pub(crate) fn check_named_cuda_benchmark_report(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    let path = requirement
        .evidence
        .iter()
        .find(|evidence| evidence.ends_with(suffix))
        .map(|evidence| resolve_manifest_path(base_dir, evidence))
        .unwrap_or_else(|| base_dir.join(suffix));
    check_single_benchmark_report(requirement, base_dir, &path, &report, true, None, failures);
    if suffix == "dataflow-analysis-release.json" {
        require_case_metric_positive(requirement, suffix, &report, "weir_nodes", failures);
        require_case_metric_positive(requirement, suffix, &report, "weir_bitset_words", failures);
    }
    if suffix == "megakernel-condition-cuda.json" {
        for metric in [
            "megakernel_condition_slots",
            "megakernel_condition_fired",
            "megakernel_condition_slots_per_sec_x1000",
        ] {
            require_case_metric_positive(requirement, suffix, &report, metric, failures);
        }
    }
    if suffix == "megakernel-latency-cuda.json" {
        for metric in [
            "megakernel_slots",
            "megakernel_dispatch_latency_ns",
            "megakernel_slots_per_sec_x1000",
            "megakernel_roundtrip_buffers",
            "megakernel_speculation_samples",
            "megakernel_speculation_adopted",
            "megakernel_speculation_rejected",
            "megakernel_speculation_side_compile_cost_ns",
            "megakernel_speculation_autotune_records",
        ] {
            require_case_metric_positive(requirement, suffix, &report, metric, failures);
        }
    }
}
pub(crate) fn check_json_value_has_no_blockers(
    requirement: &Requirement,
    label: &str,
    report: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    match report.get("blockers").and_then(serde_json::Value::as_array) {
        Some(blockers) if !blockers.is_empty() => failures.push(format!(
            "requirement `{}` {label} reports {} blocker(s)",
            requirement.id,
            blockers.len()
        )),
        Some(_) => {}
        None => failures.push(format!(
            "requirement `{}` {label} is missing blockers array",
            requirement.id
        )),
    }
}
pub(crate) fn check_json_evidence_has_no_blockers(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    check_json_value_has_no_blockers(
        requirement,
        &format!("evidence `{suffix}`"),
        &report,
        failures,
    );
}
pub(crate) fn check_marker_evidence_has_markers(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    let markers = report
        .get("markers")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if markers == 0 {
        failures.push(format!(
            "requirement `{}` marker evidence `{suffix}` contains zero markers",
            requirement.id
        ));
    }
    for required in required_marker_ids_for_suffix(suffix) {
        if !report
            .get("markers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|markers| {
                markers.iter().any(|marker| {
                    marker.get("id").and_then(serde_json::Value::as_str) == Some(required)
                })
            })
        {
            failures.push(format!(
                "requirement `{}` marker evidence `{suffix}` is missing required marker `{required}`",
                requirement.id
            ));
        }
    }
    let source_matrix = report
        .get("source_matrix")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !source_matrix.ends_with("optimization-integration-matrix.json") {
        failures.push(format!(
            "requirement `{}` marker evidence `{suffix}` does not reference optimization-integration-matrix.json",
            requirement.id
        ));
    }
}

#[cfg(test)]
mod part4_tests {
    use super::*;

    #[test]
    fn before_after_benchmark_report_rejects_explicit_blockers() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for before/after blocker gate test.");
        std::fs::write(
            dir.path().join("alias-aware-before-after.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "blockers": ["before/after benchmark reused stale CUDA baseline"],
                "selected_backend": "cuda",
                "summary": {"failed": 0},
                "cases": [
                    {
                        "id": "lower.alias_aware_optimizations",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 1, "p95": 2, "p99": 3},
                            "baseline_wall_ns": {"samples": 30, "p50": 2, "p95": 3, "p99": 4}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize before/after blocker evidence."),
        )
        .expect("Fix: write before/after blocker evidence.");
        let requirement = Requirement {
            id: "optimization-integration".to_string(),
            title: "optimization integration".to_string(),
            status: "required".to_string(),
            evidence: vec!["alias-aware-before-after.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_before_after_benchmark_report(
            &requirement,
            dir.path(),
            "alias-aware-before-after.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `optimization-integration` benchmark `alias-aware-before-after.json` reports 1 blocker(s)"
            )),
            "Fix: before/after benchmark release gate must reject explicit benchmark blockers; failures={failures:?}"
        );
    }

    #[test]
    fn json_evidence_gate_rejects_missing_blockers_array() {
        let requirement = Requirement {
            id: "cuda-first-path".to_string(),
            title: "CUDA first path".to_string(),
            status: "required".to_string(),
            evidence: vec!["bench-release-axes.json".to_string()],
            minimum_evidence: 0,
        };
        let report = serde_json::json!({
            "schema_version": 1,
            "source_artifacts": []
        });
        let mut failures = Vec::new();

        check_json_value_has_no_blockers(
            &requirement,
            "evidence `bench-release-axes.json`",
            &report,
            &mut failures,
        );

        assert_eq!(
            failures,
            vec![
                "requirement `cuda-first-path` evidence `bench-release-axes.json` is missing blockers array"
                    .to_string()
            ],
            "Fix: release gate must fail closed when JSON evidence omits its blockers array."
        );
    }

    #[test]
    fn before_after_benchmark_report_rejects_duplicate_case_identity() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for duplicate before/after case gate test.");
        std::fs::write(
            dir.path().join("alias-aware-before-after.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "summary": {"total_cases": 2, "passed": 2, "failed": 0},
                "cases": [
                    {
                        "id": "lower.alias_aware_optimizations",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 1, "p95": 2, "p99": 3},
                            "baseline_wall_ns": {"samples": 30, "p50": 2, "p95": 3, "p99": 4}
                        }
                    },
                    {
                        "id": "lower.alias_aware_optimizations",
                        "backend_id": "cuda",
                        "status": "pass",
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 1, "p95": 2, "p99": 3},
                            "baseline_wall_ns": {"samples": 30, "p50": 2, "p95": 3, "p99": 4}
                        }
                    }
                ]
            }))
            .expect("Fix: serialize duplicate before/after case evidence."),
        )
        .expect("Fix: write duplicate before/after case evidence.");
        let requirement = Requirement {
            id: "optimization-integration".to_string(),
            title: "optimization integration".to_string(),
            status: "required".to_string(),
            evidence: vec!["alias-aware-before-after.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_before_after_benchmark_report(
            &requirement,
            dir.path(),
            "alias-aware-before-after.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `optimization-integration` benchmark `alias-aware-before-after.json` has 2 cases with id `lower.alias_aware_optimizations`"
            )),
            "Fix: before/after benchmark gate must reject duplicate case ids before duplicate rows can prove optimization coverage; failures={failures:?}"
        );
    }

    #[test]
    fn before_after_benchmark_report_rejects_missing_source_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for before/after provenance gate test.");
        std::fs::write(
            dir.path().join("alias-aware-before-after.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "cuda",
                "environment": {"host_cpu_model": "test cpu"},
                "summary": {"total_cases": 1, "passed": 1, "failed": 0, "cache_hit_rate": null},
                "cases": [
                    {
                        "id": "lower.alias_aware_optimizations",
                        "backend_id": "cuda",
                        "status": "pass",
                        "dataset_fingerprint": "sha256:alias-aware-corpus",
                        "correctness": {"oracle": "before-after-equivalence"},
                        "optimization_passes": ["alias-aware-optimizations"],
                        "contract": {
                            "baselines": [
                                {
                                    "class": "CpuSota",
                                    "backend_ids": ["cuda"],
                                    "min_speedup_x": 1.01
                                }
                            ]
                        },
                        "metrics": {
                            "wall_ns": {"samples": 30, "p50": 1, "p95": 2, "p99": 3},
                            "baseline_wall_ns": {"samples": 30, "p50": 2, "p95": 3, "p99": 4},
                            "host_to_device_bytes": 128,
                            "device_to_host_bytes": 128,
                            "kernel_launches": 1
                        },
                        "performance": {"contract_passed": true}
                    }
                ]
            }))
            .expect("Fix: serialize missing before/after provenance evidence."),
        )
        .expect("Fix: write missing before/after provenance evidence.");
        let requirement = Requirement {
            id: "optimization-integration".to_string(),
            title: "optimization integration".to_string(),
            status: "required".to_string(),
            evidence: vec!["alias-aware-before-after.json".to_string()],
            minimum_evidence: 0,
        };
        let mut failures = Vec::new();

        check_before_after_benchmark_report(
            &requirement,
            dir.path(),
            "alias-aware-before-after.json",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `optimization-integration` benchmark `alias-aware-before-after.json` must include source_fingerprint provenance"
            )),
            "Fix: before/after benchmark gate must reject timing evidence without source provenance; failures={failures:?}"
        );
    }
}

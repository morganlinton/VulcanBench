use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(
        requirement,
        base_dir,
        "optimization-integration-matrix.json",
        failures,
    ) else {
        return;
    };
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if blockers != 0 {
        failures.push(format!(
            "requirement `{}` optimization matrix still reports {blockers} blocker(s)",
            requirement.id
        ));
    }
    match requirement.id.as_str() {
        "optimization-benchmark-proof" => {
            check_before_after_benchmark_report(
                requirement,
                base_dir,
                "lower-rewrite-impact-before-after.json",
                failures,
            );
            check_before_after_benchmark_report(
                requirement,
                base_dir,
                "optimizer-impact-cuda.json",
                failures,
            );
            check_before_after_benchmark_report(
                requirement,
                base_dir,
                "pass-family-benchmarks.json",
                failures,
            );
            check_json_evidence_has_no_blockers(
                requirement,
                base_dir,
                "pass-family-benchmark-manifest.json",
                failures,
            );
            if let Some(manifest) = first_json_evidence(
                requirement,
                base_dir,
                "pass-family-benchmark-manifest.json",
                failures,
            ) {
                if manifest.get("backend").and_then(serde_json::Value::as_str) != Some("cuda") {
                    failures.push(
                        "requirement `optimization-benchmark-proof` pass-family benchmark manifest must be cuda"
                            .to_string(),
                    );
                }
                let cases = manifest
                    .get("cases")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for required_family in REQUIRED_BENCHMARKED_OPTIMIZATION_FAMILIES {
                    let covered = manifest
                        .get("covered_pass_families")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|families| {
                            families
                                .iter()
                                .any(|family| family.as_str() == Some(required_family))
                        });
                    if !covered {
                        failures.push(format!(
                            "requirement `optimization-benchmark-proof` pass-family manifest does not benchmark required family `{required_family}`"
                        ));
                    }
                }
                if manifest
                    .get("uncovered_pass_families")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|families| !families.is_empty())
                {
                    failures.push(
                        "requirement `optimization-benchmark-proof` pass-family manifest reports uncovered pass families"
                            .to_string(),
                    );
                }
                for required_case in [
                    "lower.rewrites.impact.corpus",
                    "foundation.optimizer.impact",
                    "lower.egraph_saturation",
                    "lower.alias_aware_optimizations",
                ] {
                    if !cases.iter().any(|case| {
                        case.get("case_id").and_then(serde_json::Value::as_str)
                            == Some(required_case)
                            && case.get("exists").and_then(serde_json::Value::as_bool) == Some(true)
                            && case
                                .get("read_error")
                                .is_some_and(serde_json::Value::is_null)
                            && case
                                .get("required_custom_metrics")
                                .and_then(serde_json::Value::as_array)
                                .is_some_and(|metrics| !metrics.is_empty())
                            && case
                                .get("required_positive_metrics")
                                .and_then(serde_json::Value::as_array)
                                .is_some_and(|metrics| !metrics.is_empty())
                    }) {
                        failures.push(format!(
                            "requirement `optimization-benchmark-proof` pass-family manifest is missing `{required_case}`"
                        ));
                    }
                }
                for case in &cases {
                    let Some(artifact) = case.get("artifact").and_then(serde_json::Value::as_str)
                    else {
                        failures.push(
                            "requirement `optimization-benchmark-proof` pass-family manifest case is missing artifact"
                                .to_string(),
                        );
                        continue;
                    };
                    if case
                        .get("covered_pass_families")
                        .and_then(serde_json::Value::as_array)
                        .is_none_or(|families| families.is_empty())
                    {
                        failures.push(
                            "requirement `optimization-benchmark-proof` pass-family manifest case lists no covered_pass_families"
                                .to_string(),
                        );
                    }
                    for field in [
                        "missing_custom_metrics",
                        "non_positive_required_metrics",
                        "non_winning_cases",
                        "blockers",
                    ] {
                        if case
                            .get(field)
                            .and_then(serde_json::Value::as_array)
                            .is_none_or(|items| !items.is_empty())
                        {
                            failures.push(format!(
                                "requirement `optimization-benchmark-proof` pass-family manifest case `{}` has non-empty `{field}`",
                                case.get("case_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("<unknown>")
                            ));
                        }
                    }
                    let read_error = case.get("read_error");
                    if !read_error.is_some_and(serde_json::Value::is_null) {
                        failures.push(format!(
                            "requirement `optimization-benchmark-proof` pass-family manifest case `{}` read_error={}",
                            case.get("case_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("<unknown>"),
                            read_error
                                .map(serde_json::Value::to_string)
                                .unwrap_or_else(|| "<missing>".to_string())
                        ));
                    }
                    for field in ["min_wall_samples", "min_baseline_wall_samples"] {
                        if case
                            .get(field)
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0)
                            < 30
                        {
                            failures.push(format!(
                                "requirement `optimization-benchmark-proof` pass-family manifest case `{}` has `{field}` below 30",
                                case.get("case_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("<unknown>")
                            ));
                        }
                    }
                    for field in [
                        "min_wall_p50",
                        "min_wall_p95",
                        "min_wall_p99",
                        "min_baseline_wall_p50",
                        "min_baseline_wall_p95",
                        "min_baseline_wall_p99",
                    ] {
                        if case
                            .get(field)
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0)
                            == 0
                        {
                            failures.push(format!(
                                "requirement `optimization-benchmark-proof` pass-family manifest case `{}` has non-positive `{field}`",
                                case.get("case_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("<unknown>")
                            ));
                        }
                    }
                    let has_speed_win = case
                        .get("min_wall_speedup_x1000")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        > 1_000;
                    let has_semantic_win = case
                        .get("non_winning_cases")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|items| items.is_empty());
                    if !has_speed_win && !has_semantic_win {
                        failures.push(format!(
                            "requirement `optimization-benchmark-proof` pass-family manifest case `{}` does not prove optimized wall_ns p50 beats baseline_wall_ns p50",
                            case.get("case_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("<unknown>")
                        ));
                    }
                    let Some(report) =
                        read_json_artifact_ref(requirement, base_dir, artifact, failures)
                    else {
                        continue;
                    };
                    let suffix = Path::new(artifact)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(artifact);
                    if let Some(metrics) = case
                        .get("required_custom_metrics")
                        .and_then(serde_json::Value::as_array)
                    {
                        for metric in metrics.iter().filter_map(serde_json::Value::as_str) {
                            require_case_metric_present(
                                requirement,
                                suffix,
                                &report,
                                metric,
                                failures,
                            );
                        }
                    }
                    if let Some(metrics) = case
                        .get("required_positive_metrics")
                        .and_then(serde_json::Value::as_array)
                    {
                        for metric in metrics.iter().filter_map(serde_json::Value::as_str) {
                            require_case_metric_positive(
                                requirement,
                                suffix,
                                &report,
                                metric,
                                failures,
                            );
                        }
                    }
                }
            }
        }
        "egraph-saturation" => {
            check_json_evidence_has_no_blockers(
                requirement,
                base_dir,
                "egraph-saturation-matrix.json",
                failures,
            );
            check_marker_evidence_has_markers(
                requirement,
                base_dir,
                "egraph-saturation-matrix.json",
                failures,
            );
            check_json_evidence_has_no_blockers(
                requirement,
                base_dir,
                "egraph-semantic-contracts.json",
                failures,
            );
            check_marker_evidence_has_markers(
                requirement,
                base_dir,
                "egraph-semantic-contracts.json",
                failures,
            );
            check_before_after_benchmark_report(
                requirement,
                base_dir,
                "egraph-before-after.json",
                failures,
            );
            if let Some(report) =
                first_json_evidence(requirement, base_dir, "egraph-before-after.json", failures)
            {
                require_case_metric_positive(
                    requirement,
                    "egraph-before-after.json",
                    &report,
                    "egraph_equality_classes",
                    failures,
                );
                require_case_metric_positive(
                    requirement,
                    "egraph-before-after.json",
                    &report,
                    "egraph_bitwise_case_count",
                    failures,
                );
                require_case_metric_positive(
                    requirement,
                    "egraph-before-after.json",
                    &report,
                    "egraph_boolean_case_count",
                    failures,
                );
                require_case_metric_positive(
                    requirement,
                    "egraph-before-after.json",
                    &report,
                    "egraph_applied_rewrites",
                    failures,
                );
            }
        }
        "alias-aware-upgrades" => {
            for suffix in [
                "alias-aware-dse.json",
                "alias-aware-stlf.json",
                "alias-aware-licm.json",
                "alias-aware-fusion-fission.json",
            ] {
                check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
                check_marker_evidence_has_markers(requirement, base_dir, suffix, failures);
            }
            check_before_after_benchmark_report(
                requirement,
                base_dir,
                "alias-aware-before-after.json",
                failures,
            );
            if let Some(report) = first_json_evidence(
                requirement,
                base_dir,
                "alias-aware-before-after.json",
                failures,
            ) {
                for metric in [
                    "alias_pass_wins",
                    "alias_fact_count",
                    "alias_cross_binding_fact_count",
                    "reaching_def_fact_count",
                ] {
                    require_case_metric_positive(
                        requirement,
                        "alias-aware-before-after.json",
                        &report,
                        metric,
                        failures,
                    );
                }
            }
        }
        _ => {}
    }
}

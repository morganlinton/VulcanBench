use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(
        requirement,
        base_dir,
        "release-workload-matrix.json",
        failures,
    ) else {
        return;
    };
    let required = matrix
        .get("required_closed_families")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let matched = matrix
        .get("matched_required_families")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let release_cases = matrix
        .get("release_suite_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if required < 12 {
        failures.push(format!(
            "requirement `proof-workloads-12` matrix requires only {required} workload families; needs at least 12"
        ));
    }
    if matched < 12 {
        failures.push(format!(
            "requirement `proof-workloads-12` matrix covers {matched} workload families; needs at least 12"
        ));
    }
    if matched < required {
        failures.push(format!(
            "requirement `proof-workloads-12` matrix covers {matched} of {required} required workload families"
        ));
    }
    if release_cases < matched {
        failures.push(format!(
            "requirement `proof-workloads-12` matrix reports {release_cases} release cases for {matched} matched workload families"
        ));
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `proof-workloads-12` matrix still reports {blockers} blocker(s)"
        ));
    }
    check_release_bench_targets(requirement, base_dir, failures);
    check_workload_matrix_artifact_coverage(requirement, base_dir, &matrix, failures);
    check_benchmark_evidence_reports(requirement, base_dir, "workload-", true, None, failures);
}

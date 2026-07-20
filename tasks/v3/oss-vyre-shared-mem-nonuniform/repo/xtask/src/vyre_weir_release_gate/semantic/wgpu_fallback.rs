use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(requirement, base_dir, "backend-matrix.json", failures)
    else {
        return;
    };
    let present = matrix
        .get("wgpu_fallback_present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if !present {
        failures.push(
            "requirement `wgpu-fallback` backend matrix does not prove acquireable WGPU fallback"
                .to_string(),
        );
    }
    check_backend_matrix_schema("wgpu-fallback", &matrix, failures);
    if blockers != 0 {
        failures.push(format!(
            "requirement `wgpu-fallback` backend matrix still reports {blockers} blocker(s)"
        ));
    }
    require_no_hidden_backend_fallback_findings("wgpu-fallback", &matrix, failures);
    check_backend_gpu_probe("wgpu-fallback", &matrix, failures);
    check_preferred_backend_gpu_only("wgpu-fallback", &matrix, failures);
    check_backend_acquire_entry("wgpu-fallback", &matrix, "wgpu", failures);
    check_backend_feature_markers(
        "wgpu-fallback",
        &matrix,
        "wgpu_feature_markers",
        7,
        failures,
    );
    check_json_evidence_has_no_blockers(
        requirement,
        base_dir,
        "wgpu-fallback-suite.json",
        failures,
    );
    check_backend_suite_report(requirement, base_dir, "wgpu-fallback-suite.json", failures);
    check_backend_suite_parity(requirement, base_dir, failures);
}

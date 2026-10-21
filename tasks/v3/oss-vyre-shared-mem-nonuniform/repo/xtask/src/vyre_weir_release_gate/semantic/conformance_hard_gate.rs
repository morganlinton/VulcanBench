use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) =
        first_json_evidence(requirement, base_dir, "conformance-matrix.json", failures)
    else {
        return;
    };
    let op_count = matrix
        .get("op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let distinct_op_count = matrix
        .get("distinct_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_required_op_count = matrix
        .get("catalog_required_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_covered_op_count = matrix
        .get("catalog_covered_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_catalog_ops = matrix
        .get("missing_catalog_ops")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_blocked_release_count = matrix
        .get("op_matrix_blocked_release_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let release_backend_row_count = matrix
        .get("release_backend_row_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_release_backend_rows = matrix
        .get("missing_release_backend_rows")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_errors = matrix
        .get("op_matrix_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if op_matrix_errors != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {op_matrix_errors} OP_MATRIX read/parse error(s)"
        ));
    }
    let fixture_input_count = matrix
        .get("fixture_input_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let expected_output_count = matrix
        .get("expected_output_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if op_count == 0 {
        failures.push(
            "requirement `conformance-hard-gate` matrix contains zero op entries".to_string(),
        );
    }
    if op_count < 49 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix has {op_count} op entries, below release floor 49"
        ));
    }
    if distinct_op_count < 49 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix has {distinct_op_count} distinct op id(s), below release floor 49"
        ));
    }
    if catalog_required_op_count == 0
        || catalog_covered_op_count != catalog_required_op_count
        || missing_catalog_ops != 0
    {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix covers {catalog_covered_op_count}/{catalog_required_op_count} OP_MATRIX-required op id(s), missing_catalog_ops={missing_catalog_ops}"
        ));
    }
    if op_matrix_blocked_release_count != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {op_matrix_blocked_release_count} OP_MATRIX release backend row(s) marked blocked_release"
        ));
    }
    let expected_release_backend_rows = catalog_required_op_count.saturating_mul(3);
    if release_backend_row_count < expected_release_backend_rows
        || missing_release_backend_rows != 0
    {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix has release_backend_row_count={release_backend_row_count}, expected {expected_release_backend_rows}, missing_release_backend_rows={missing_release_backend_rows}"
        ));
    }
    if fixture_input_count != op_count {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix fixture_input_count {fixture_input_count} must equal op_count {op_count}"
        ));
    }
    if expected_output_count != op_count {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix expected_output_count {expected_output_count} must equal op_count {op_count}"
        ));
    }
    if matrix
        .get("duplicate_op_ids")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|duplicates| !duplicates.is_empty())
    {
        failures.push(
            "requirement `conformance-hard-gate` matrix reports duplicate op id(s)".to_string(),
        );
    }
    let backends = matrix
        .get("dispatch_backends")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in ["cuda", "wgpu", "cpu-ref"] {
        if !backends
            .iter()
            .any(|backend| backend.as_str() == Some(required))
        {
            failures.push(format!(
                "requirement `conformance-hard-gate` matrix dispatch_backends is missing `{required}`"
            ));
        }
    }
    let ci_gate_count = matrix
        .get("ci_blocking_gate_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let schema_version = matrix
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix schema_version is {schema_version}, expected >= 2"
        ));
    }
    if ci_gate_count < 3 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports only {ci_gate_count} blocking CI conformance gate(s), needs at least 3"
        ));
    }
    let ci_gates = matrix
        .get("ci_gates")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let required_ci_statuses = matrix
        .get("required_ci_statuses")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_ci_statuses == 0 {
        failures.push(
            "requirement `conformance-hard-gate` matrix parsed zero required CI status context(s)"
                .to_string(),
        );
    }
    let missing_required_ci_statuses = matrix
        .get("missing_required_ci_statuses")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_ci_statuses != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {missing_required_ci_statuses} required CI status context(s) missing from workflows"
        ));
    }
    let ci_status_scan_errors = matrix
        .get("ci_status_scan_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if ci_status_scan_errors != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {ci_status_scan_errors} CI status scan error(s)"
        ));
    }
    let path_filtered_required_workflows = matrix
        .get("path_filtered_required_workflows")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if path_filtered_required_workflows != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {path_filtered_required_workflows} required workflow(s) still using path filters"
        ));
    }
    let missing_required_workflow_triggers = matrix
        .get("missing_required_workflow_triggers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_workflow_triggers != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {missing_required_workflow_triggers} required workflow(s) missing pull_request + push main trigger coverage"
        ));
    }
    let missing_fail_closed_fanins = matrix
        .get("missing_fail_closed_fanins")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_fail_closed_fanins != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix reports {missing_fail_closed_fanins} required fan-in job(s) missing fail-closed dependency checks"
        ));
    }
    for required_workflow in [
        "/Santh/.github/workflows/conform.yml",
        "/Santh/.github/workflows/gpu-parity.yml",
        "/Santh/.github/workflows/santh-ci.yml",
        "/Santh/.github/workflows/architectural-invariants.yml",
        "/Santh/.github/CI_REQUIRED.md",
        "/Santh/scripts/apply-branch-protection.sh",
        "/Santh/libs/performance/matching/vyre/.github/workflows/conform.yml",
        "/Santh/libs/performance/matching/vyre/.github/workflows/gpu-parity.yml",
    ] {
        if !ci_gates.iter().any(|gate| {
            gate.get("workflow")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|workflow| workflow.ends_with(required_workflow))
                && gate.get("present").and_then(serde_json::Value::as_bool) == Some(true)
                && gate
                    .get("command_present")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && gate
                    .get("artifact_check_present")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        }) {
            failures.push(format!(
                "requirement `conformance-hard-gate` matrix is missing complete CI workflow `{required_workflow}`"
            ));
        }
    }
    for required_gate in [
        "conformance matrix release blocker",
        "gpu-release-gate",
        "conform-release-gate",
        "Vyre structural release evidence",
        "Vyre/Weir final release gate",
        "Vyre/Weir final conformance artifact download",
        "Vyre/Weir final benchmark artifact download",
        "Vyre/Weir final conformance staging",
        "Vyre/Weir final benchmark staging",
        "Vyre/Weir final optimization staging",
        "Vyre/Weir final structural evidence",
        "Vyre/Weir final completion audit",
        "vyre-weir-final-release-evidence",
        "architectural-invariants",
        "required_status_checks",
    ] {
        if !ci_gates.iter().any(|gate| {
            gate.get("gate").and_then(serde_json::Value::as_str) == Some(required_gate)
                && gate.get("present").and_then(serde_json::Value::as_bool) == Some(true)
                && gate
                    .get("command_present")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && gate
                    .get("artifact_check_present")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        }) {
            failures.push(format!(
                "requirement `conformance-hard-gate` matrix is missing complete CI gate `{required_gate}`"
            ));
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `conformance-hard-gate` matrix still reports {blockers} blocker(s)"
        ));
    }
    for suffix in [
        "cuda-conformance.json",
        "wgpu-conformance.json",
        "reference-conformance.json",
        "release-gate-log.json",
    ] {
        check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
    }
    for suffix in [
        "cuda-conformance.json",
        "wgpu-conformance.json",
        "reference-conformance.json",
    ] {
        check_backend_conformance_report(requirement, base_dir, suffix, failures);
    }
    if let Some(log) = first_json_evidence(requirement, base_dir, "release-gate-log.json", failures)
    {
        let schema_version = log
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if schema_version < 2 {
            failures.push(format!(
                "requirement `conformance-hard-gate` release log schema_version={schema_version}; expected schema>=2"
            ));
        }
        let requested = log
            .get("requested_backends")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for backend in ["cuda", "wgpu", "cpu-ref"] {
            if !requested
                .iter()
                .any(|entry| entry.as_str() == Some(backend))
            {
                failures.push(format!(
                    "requirement `conformance-hard-gate` release log requested_backends is missing `{backend}`"
                ));
            }
        }
        let statuses = log
            .get("artifact_statuses")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for artifact in [
            "cuda-conformance.json",
            "wgpu-conformance.json",
            "reference-conformance.json",
        ] {
            if !statuses.iter().any(|status| {
                status.get("path").and_then(serde_json::Value::as_str) == Some(artifact)
                    && status.get("exists").and_then(serde_json::Value::as_bool) == Some(true)
                    && status
                        .get("bytes")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        > 0
                    && status
                        .get("read_error")
                        .is_some_and(serde_json::Value::is_null)
            }) {
                failures.push(format!(
                    "requirement `conformance-hard-gate` release log does not prove non-empty readable artifact `{artifact}`"
                ));
            }
        }
    }
}

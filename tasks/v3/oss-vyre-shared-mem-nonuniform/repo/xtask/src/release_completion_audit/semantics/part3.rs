fn inspect_conformance_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let op_count = value
        .get("op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let distinct_op_count = value
        .get("distinct_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_required_op_count = value
        .get("catalog_required_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_covered_op_count = value
        .get("catalog_covered_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_catalog_ops = value
        .get("missing_catalog_ops")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_blocked_release_count = value
        .get("op_matrix_blocked_release_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let op_matrix_errors = value
        .get("op_matrix_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if op_matrix_errors != 0 {
        blockers.push(format!(
            "{evidence}: reports {op_matrix_errors} OP_MATRIX read/parse error(s)"
        ));
    }
    inspect_named_blockers(evidence, value, "conformance matrix", blockers);
    let fixture_input_count = value
        .get("fixture_input_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let expected_output_count = value
        .get("expected_output_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if op_count < 49 {
        blockers.push(format!(
            "{evidence}: op_count is {op_count}, below release floor 49"
        ));
    }
    if distinct_op_count < 49 {
        blockers.push(format!(
            "{evidence}: distinct_op_count is {distinct_op_count}, below release floor 49"
        ));
    }
    if catalog_required_op_count == 0
        || catalog_covered_op_count != catalog_required_op_count
        || missing_catalog_ops != 0
    {
        blockers.push(format!(
            "{evidence}: covers {catalog_covered_op_count}/{catalog_required_op_count} OP_MATRIX-required op id(s), missing_catalog_ops={missing_catalog_ops}"
        ));
    }
    if op_matrix_blocked_release_count != 0 {
        blockers.push(format!(
            "{evidence}: op_matrix_blocked_release_count must be zero, got {op_matrix_blocked_release_count}"
        ));
    }
    if fixture_input_count != op_count {
        blockers.push(format!(
            "{evidence}: fixture_input_count {fixture_input_count} must equal op_count {op_count}"
        ));
    }
    if expected_output_count != op_count {
        blockers.push(format!(
            "{evidence}: expected_output_count {expected_output_count} must equal op_count {op_count}"
        ));
    }
    if value
        .get("duplicate_op_ids")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|duplicates| !duplicates.is_empty())
    {
        blockers.push(format!("{evidence}: duplicate_op_ids must be empty"));
    }
    let backends = value
        .get("dispatch_backends")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in ["cuda", "wgpu", "cpu-ref"] {
        if !backends
            .iter()
            .any(|backend| backend.as_str() == Some(required))
        {
            blockers.push(format!(
                "{evidence}: dispatch_backends must include `{required}`"
            ));
        }
    }
    let ci_gate_count = value
        .get("ci_blocking_gate_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        blockers.push(format!(
            "{evidence}: schema_version is {schema_version}, expected >= 2"
        ));
    }
    if ci_gate_count < 3 {
        blockers.push(format!(
            "{evidence}: ci_blocking_gate_count is {ci_gate_count}, needs at least 3"
        ));
    }
    let ci_gates = value
        .get("ci_gates")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let required_ci_statuses = value
        .get("required_ci_statuses")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_ci_statuses == 0 {
        blockers.push(format!(
            "{evidence}: parsed zero required CI status context(s)"
        ));
    }
    let missing_required_ci_statuses = value
        .get("missing_required_ci_statuses")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_ci_statuses != 0 {
        blockers.push(format!(
            "{evidence}: {missing_required_ci_statuses} required CI status context(s) are missing from workflows"
        ));
    }
    let ci_status_scan_errors = value
        .get("ci_status_scan_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if ci_status_scan_errors != 0 {
        blockers.push(format!(
            "{evidence}: {ci_status_scan_errors} CI status scan error(s) make workflow status evidence incomplete"
        ));
    }
    let path_filtered_required_workflows = value
        .get("path_filtered_required_workflows")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if path_filtered_required_workflows != 0 {
        blockers.push(format!(
            "{evidence}: {path_filtered_required_workflows} required workflow(s) still use path filters"
        ));
    }
    let missing_required_workflow_triggers = value
        .get("missing_required_workflow_triggers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_workflow_triggers != 0 {
        blockers.push(format!(
            "{evidence}: {missing_required_workflow_triggers} required workflow(s) are missing pull_request + push main trigger coverage"
        ));
    }
    let missing_fail_closed_fanins = value
        .get("missing_fail_closed_fanins")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_fail_closed_fanins != 0 {
        blockers.push(format!(
            "{evidence}: {missing_fail_closed_fanins} required fan-in job(s) are missing fail-closed dependency checks"
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
            blockers.push(format!(
                "{evidence}: missing complete CI conformance workflow `{required_workflow}`"
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
            blockers.push(format!(
                "{evidence}: missing complete CI conformance gate `{required_gate}`"
            ));
        }
    }
}

fn inspect_release_conformance_log_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        blockers.push(format!(
            "{evidence}: schema_version={schema_version}; release conformance log must be schema>=2"
        ));
    }
    inspect_named_blockers(evidence, value, "release conformance log", blockers);
    if !value
        .get("command")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|command| {
            command.contains("cargo_full") && command.contains("release-conformance")
        })
    {
        blockers.push(format!(
            "{evidence}: command must run release-conformance through cargo_full"
        ));
    }
    let requested = value
        .get("requested_backends")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for backend in ["cuda", "wgpu", "cpu-ref"] {
        if !requested
            .iter()
            .any(|entry| entry.as_str() == Some(backend))
        {
            blockers.push(format!(
                "{evidence}: requested_backends is missing `{backend}`"
            ));
        }
    }
    let statuses = value
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
            blockers.push(format!(
                "{evidence}: does not prove non-empty readable conformance artifact `{artifact}`"
            ));
        }
    }
}

fn inspect_named_blockers(
    evidence: &str,
    value: &serde_json::Value,
    surface: &str,
    blockers: &mut Vec<String>,
) {
    if let Some(surface_blockers) = value.get("blockers").and_then(serde_json::Value::as_array) {
        for blocker in surface_blockers {
            blockers.push(format!(
                "{evidence}: {surface} blocker: {}",
                blocker.as_str().unwrap_or("<non-string blocker>")
            ));
        }
    }
}

#[cfg(test)]
mod part3_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_conformance_matrix_blockers() {
        let matrix = serde_json::json!({
            "schema_version": 2,
            "op_count": 49,
            "distinct_op_count": 49,
            "catalog_required_op_count": 49,
            "catalog_covered_op_count": 49,
            "missing_catalog_ops": [],
            "op_matrix_blocked_release_count": 0,
            "op_matrix_errors": [],
            "fixture_input_count": 49,
            "expected_output_count": 49,
            "duplicate_op_ids": [],
            "dispatch_backends": ["cuda", "wgpu", "cpu-ref"],
            "ci_blocking_gate_count": 3,
            "required_ci_statuses": ["conform"],
            "missing_required_ci_statuses": [],
            "ci_status_scan_errors": [],
            "path_filtered_required_workflows": [],
            "missing_required_workflow_triggers": [],
            "missing_fail_closed_fanins": [],
            "ci_gates": [],
            "blockers": ["CUDA conformance artifact failed"]
        });
        let mut blockers = Vec::new();

        inspect_conformance_matrix_semantics("conformance-matrix.json", &matrix, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "conformance-matrix.json: conformance matrix blocker: CUDA conformance artifact failed"
            )),
            "Fix: completion audit must reject explicit conformance matrix blockers; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_release_conformance_log_blockers() {
        let log = serde_json::json!({
            "schema_version": 2,
            "command": "cargo_full run --bin xtask -- release-conformance",
            "requested_backends": ["cuda", "wgpu", "cpu-ref"],
            "artifact_statuses": [
                {"path": "cuda-conformance.json", "exists": true, "bytes": 1, "read_error": null},
                {"path": "wgpu-conformance.json", "exists": true, "bytes": 1, "read_error": null},
                {"path": "reference-conformance.json", "exists": true, "bytes": 1, "read_error": null}
            ],
            "blockers": ["wgpu conformance produced zero op pairs"]
        });
        let mut blockers = Vec::new();

        inspect_release_conformance_log_semantics("release-gate-log.json", &log, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release-gate-log.json: release conformance log blocker: wgpu conformance produced zero op pairs"
            )),
            "Fix: completion audit must reject explicit release conformance log blockers; blockers={blockers:?}"
        );
    }
}

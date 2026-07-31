use super::*;

#[test]
fn release_conformance_artifacts_prove_three_backend_catalog_completeness() {
    let gate = repo_json("release/evidence/conformance/release-gate-log.json");
    assert_eq!(
        gate["schema_version"], 2,
        "Fix: release gate log schema must be v2."
    );
    assert_json_string_array_contains_exactly(
        &gate["requested_backends"],
        &["cuda", "wgpu", "cpu-ref"],
        "requested_backends",
    );
    assert_json_string_array_contains_exactly(
        &gate["required_artifacts"],
        &[
            "cuda-conformance.json",
            "wgpu-conformance.json",
            "reference-conformance.json",
        ],
        "required_artifacts",
    );
    assert!(
        gate["blockers"].as_array().is_some_and(Vec::is_empty),
        "Fix: release conformance gate must have zero blockers."
    );
    for status in gate["artifact_statuses"]
        .as_array()
        .expect("Fix: release gate log must contain artifact_statuses")
    {
        let path = status["path"]
            .as_str()
            .expect("Fix: conformance artifact status must name a path");
        assert_eq!(
            status["exists"], true,
            "Fix: required conformance artifact `{path}` must exist."
        );
        assert!(
            status["bytes"].as_u64().unwrap_or(0) > 1000,
            "Fix: required conformance artifact `{path}` is too small to be a real certificate."
        );
        assert!(
            status["read_error"].is_null(),
            "Fix: required conformance artifact `{path}` must be readable."
        );
    }

    let matrix = repo_json("release/evidence/conformance/conformance-matrix.json");
    let matrix_summary = ConformanceSummary::from_json(&matrix, "conformance-matrix.json");
    assert!(
        matrix_summary.distinct_op_count >= 400,
        "Fix: release conformance matrix must cover the full catalog-scale op surface."
    );
    assert_eq!(
        matrix_summary.catalog_required_op_count, matrix_summary.catalog_covered_op_count,
        "Fix: every required catalog op must be covered by release conformance rows."
    );
    assert!(
        matrix_summary.missing_catalog_ops.is_empty(),
        "Fix: release conformance matrix has missing catalog ops: {:?}",
        matrix_summary.missing_catalog_ops
    );
    assert_eq!(
        matrix_summary.release_backend_row_count,
        matrix_summary.catalog_required_op_count * 3,
        "Fix: release conformance matrix must contain exactly one reference, CUDA, and WGPU row for every required catalog op."
    );
    let expected_rows = release_backend_rows(&matrix, "conformance-matrix.json");
    assert_eq!(
        expected_rows.len(),
        matrix_summary.release_backend_row_count,
        "Fix: release conformance row count field must match release_backend_rows length."
    );
    assert_complete_backend_rows(&expected_rows, matrix_summary.catalog_required_op_count);

    for (backend_id, artifact) in [
        ("cuda", "cuda-conformance.json"),
        ("wgpu", "wgpu-conformance.json"),
        ("cpu-ref", "reference-conformance.json"),
    ] {
        let artifact_path = format!("release/evidence/conformance/{artifact}");
        let json = repo_json(&artifact_path);
        let summary = ConformanceSummary::from_json(&json, &artifact_path);
        assert_eq!(
            json["backend_id"], backend_id,
            "Fix: `{artifact}` must declare backend_id `{backend_id}`."
        );
        let command = json["command"]
            .as_str()
            .expect("Fix: conformance artifact must record the command that generated it");
        assert!(
            command.contains("cargo_full")
                && command.contains("vyre-conform-runner")
                && command.contains("dispatch --backend")
                && command.contains(backend_id),
            "Fix: `{artifact}` must record a reproducible cargo_full dispatch command for `{backend_id}`, got `{command}`."
        );
        assert_eq!(
            summary.distinct_op_count, matrix_summary.distinct_op_count,
            "Fix: `{artifact}` distinct_op_count must agree with conformance-matrix.json."
        );
        assert_eq!(
            summary.catalog_required_op_count, matrix_summary.catalog_required_op_count,
            "Fix: `{artifact}` catalog_required_op_count must agree with conformance-matrix.json."
        );
        assert_eq!(
            summary.catalog_covered_op_count, matrix_summary.catalog_covered_op_count,
            "Fix: `{artifact}` catalog_covered_op_count must agree with conformance-matrix.json."
        );
        assert!(
            summary.missing_catalog_ops.is_empty(),
            "Fix: `{artifact}` reports missing catalog ops: {:?}",
            summary.missing_catalog_ops
        );
        assert!(
            json["stdout_diagnostics"]
                .as_array()
                .is_some_and(Vec::is_empty),
            "Fix: `{artifact}` must not carry ignored stdout diagnostics."
        );
        assert_conformance_artifact_has_no_failures(&json, artifact);
        assert_runtime_dialect_rows(&json, backend_id, artifact);
        let rows = release_backend_rows(&json, &artifact_path);
        assert_eq!(
            rows, expected_rows,
            "Fix: `{artifact}` release backend rows must match conformance-matrix.json exactly."
        );
    }
}


pub(crate) fn required_marker_ids_for_suffix(suffix: &str) -> &'static [&'static str] {
    if suffix == "alias-aware-dse.json" {
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
        ]
    } else if suffix == "alias-aware-stlf.json" {
        &[
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
            "dataflow-analysis-stlf-firing-test",
        ]
    } else if suffix == "alias-aware-licm.json" {
        &[
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
        ]
    } else if suffix == "alias-aware-fusion-fission.json" {
        &[
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
        ]
    } else if suffix == "weir-facts-pass-firing.json" {
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
        ]
    } else if suffix == "egraph-saturation-matrix.json"
        || suffix == "egraph-semantic-contracts.json"
    {
        &[
            "egraph-saturation",
            "egraph-canonical-pipeline-entrypoint",
            "egraph-algebraic-reassociation",
            "egraph-bitwise-reassociation",
        ]
    } else {
        &[]
    }
}
pub(crate) fn check_backend_feature_marker_id(
    requirement_id: &str,
    matrix: &serde_json::Value,
    field: &str,
    required_id: &str,
    failures: &mut Vec<String>,
) {
    let Some(markers) = matrix.get(field).and_then(serde_json::Value::as_array) else {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix is missing `{field}`"
        ));
        return;
    };
    let Some(marker) = markers
        .iter()
        .find(|marker| marker.get("id").and_then(serde_json::Value::as_str) == Some(required_id))
    else {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix `{field}` is missing required marker `{required_id}`"
        ));
        return;
    };
    if marker.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix `{field}` marker `{required_id}` does not exist"
        ));
    }
    let missing_tokens = marker
        .get("missing_tokens")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_tokens != 0 {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix `{field}` marker `{required_id}` reports {missing_tokens} missing token(s)"
        ));
    }
    let unresolved_markers = marker
        .get("unresolved_markers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if unresolved_markers != 0 {
        failures.push(format!(
            "requirement `{requirement_id}` backend matrix `{field}` marker `{required_id}` reports {unresolved_markers} unresolved marker(s)"
        ));
    }
}
pub(crate) fn check_parser_contract_evidence(
    requirement: &Requirement,
    base_dir: &Path,
    suffix: &str,
    failures: &mut Vec<String>,
) {
    let Some(report) = first_json_evidence(requirement, base_dir, suffix, failures) else {
        return;
    };
    let expected_component = if suffix == "vyrec-cli-contracts.json" {
        "vyrec"
    } else {
        suffix.strip_suffix("-contracts.json").unwrap_or(suffix)
    };
    let component_id = report
        .get("component_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if component_id != expected_component {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has component_id `{component_id}`, expected `{expected_component}`",
            requirement.id
        ));
    }
    if report
        .get("role")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|role| role.is_empty())
    {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has an empty role",
            requirement.id
        ));
    }
    if report
        .get("root")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|root| root.is_empty())
    {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has an empty root",
            requirement.id
        ));
    }
    let required_terms = report
        .get("required_terms")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_terms == 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has no required terms",
            requirement.id
        ));
    }
    let missing_terms = report
        .get("missing_terms")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_terms != 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` reports {missing_terms} missing term(s)",
            requirement.id
        ));
    }
    let required_contract_topics = report
        .get("required_contract_topics")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_contract_topics == 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has no required contract topics",
            requirement.id
        ));
    }
    let missing_contract_topics = report
        .get("missing_contract_topics")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_contract_topics != 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` reports {missing_contract_topics} missing contract topic(s)",
            requirement.id
        ));
    }
    let required_test_categories = report
        .get("required_test_categories")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_test_categories == 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has no required test categories",
            requirement.id
        ));
    }
    let missing_test_categories = report
        .get("missing_test_categories")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_test_categories != 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` reports {missing_test_categories} missing test categor(ies)",
            requirement.id
        ));
    }
    let required_evidence_trees = report
        .get("required_evidence_trees")
        .and_then(serde_json::Value::as_array);
    if required_evidence_trees.is_none_or(|trees| trees.len() < 3) {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` must list tests, benches, and fuzz evidence trees",
            requirement.id
        ));
    }
    check_duplicate_parser_contract_object_rows(
        requirement,
        suffix,
        &report,
        "required_evidence_trees",
        "tree",
        failures,
    );
    if let Some(trees) = required_evidence_trees {
        for tree in trees {
            let tree_name = tree
                .get("tree")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if tree.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
                failures.push(format!(
                    "requirement `{}` parser contract `{suffix}` evidence tree `{tree_name}` does not exist",
                    requirement.id
                ));
            }
            let source_bytes = tree
                .get("source_bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if source_bytes == 0 {
                failures.push(format!(
                    "requirement `{}` parser contract `{suffix}` evidence tree `{tree_name}` has zero source bytes",
                    requirement.id
                ));
            }
            let unreadable = tree
                .get("unreadable_file_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX);
            if unreadable != 0 {
                failures.push(format!(
                    "requirement `{}` parser contract `{suffix}` evidence tree `{tree_name}` has {unreadable} unreadable source file(s)",
                    requirement.id
                ));
            }
        }
    }
    let unresolved_ownership_markers = report
        .get("unresolved_ownership_markers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if unresolved_ownership_markers != 0 {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` reports {unresolved_ownership_markers} unresolved ownership marker(s)",
            requirement.id
        ));
    }
    let Some(required_files) = report
        .get("required_files")
        .and_then(serde_json::Value::as_array)
    else {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has no required_files array",
            requirement.id
        ));
        return;
    };
    if required_files.is_empty() {
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has zero required file(s)",
            requirement.id
        ));
    }
    check_duplicate_parser_contract_object_rows(
        requirement,
        suffix,
        &report,
        "required_files",
        "path",
        failures,
    );
    for file in required_files {
        let path = file
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if file.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
            failures.push(format!(
                "requirement `{}` parser contract `{suffix}` required file `{path}` does not exist",
                requirement.id
            ));
        }
        if file
            .get("source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(format!(
                "requirement `{}` parser contract `{suffix}` required file `{path}` is empty",
                requirement.id
            ));
        }
        let read_error = file.get("read_error");
        if !read_error.is_some_and(serde_json::Value::is_null) {
            failures.push(format!(
                "requirement `{}` parser contract `{suffix}` required file `{path}` read_error={}",
                requirement.id,
                read_error
                    .map(serde_json::Value::to_string)
                    .unwrap_or_else(|| "<missing>".to_string())
            ));
        }
    }
}

fn check_duplicate_parser_contract_object_rows(
    requirement: &Requirement,
    suffix: &str,
    report: &serde_json::Value,
    array_field: &str,
    object_field: &str,
    failures: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            report,
            array_field,
            object_field,
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        failures.push(format!(
            "requirement `{}` parser contract `{suffix}` has duplicate {array_field}.{object_field} rows: {duplicates}",
            requirement.id
        ));
    }
}
pub(crate) fn check_backend_conformance_report(
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
            "requirement `{}` backend conformance `{suffix}` schema_version={schema_version}; expected schema>=2",
            requirement.id
        ));
    }
    let expected_backend = match suffix {
        "cuda-conformance.json" => Some("cuda"),
        "wgpu-conformance.json" => Some("wgpu"),
        "reference-conformance.json" => Some("cpu-ref"),
        _ => None,
    };
    if let Some(expected) = expected_backend {
        let backend_id = report.get("backend_id").and_then(serde_json::Value::as_str);
        if backend_id != Some(expected) {
            failures.push(format!(
                "requirement `{}` backend conformance `{suffix}` reports backend `{:?}`, expected `{expected}`",
                requirement.id,
                backend_id
            ));
        }
    }
    let total_pairs = report
        .get("total_pairs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let failed_pairs = report
        .get("failed_pairs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let distinct_op_count = report
        .get("distinct_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_required_op_count = report
        .get("catalog_required_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let catalog_covered_op_count = report
        .get("catalog_covered_op_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_catalog_ops = report
        .get("missing_catalog_ops")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_blocked_release_count = report
        .get("op_matrix_blocked_release_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let release_backend_row_count = report
        .get("release_backend_row_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_release_backend_rows = report
        .get("missing_release_backend_rows")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_errors = report
        .get("op_matrix_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if op_matrix_errors != 0 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports {op_matrix_errors} OP_MATRIX read/parse error(s)",
            requirement.id
        ));
    }
    if total_pairs == 0 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports zero op pairs",
            requirement.id
        ));
    }
    if total_pairs < 49 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports {total_pairs} op pair(s), below release floor 49",
            requirement.id
        ));
    }
    if distinct_op_count < 49 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports {distinct_op_count} distinct op id(s), below release floor 49",
            requirement.id
        ));
    }
    if catalog_required_op_count == 0
        || catalog_covered_op_count != catalog_required_op_count
        || missing_catalog_ops != 0
    {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` covers {catalog_covered_op_count}/{catalog_required_op_count} OP_MATRIX-required op id(s), missing_catalog_ops={missing_catalog_ops}",
            requirement.id
        ));
    }
    if op_matrix_blocked_release_count != 0 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports {op_matrix_blocked_release_count} OP_MATRIX release backend row(s) marked blocked_release",
            requirement.id
        ));
    }
    let expected_release_backend_rows = catalog_required_op_count.saturating_mul(3);
    if release_backend_row_count < expected_release_backend_rows
        || missing_release_backend_rows != 0
    {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` has release_backend_row_count={release_backend_row_count}, expected {expected_release_backend_rows}, missing_release_backend_rows={missing_release_backend_rows}",
            requirement.id
        ));
    }
    if failed_pairs != 0 {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports {failed_pairs} failed pair(s)",
            requirement.id
        ));
    }
    if report
        .get("duplicate_op_ids")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|duplicates| !duplicates.is_empty())
    {
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` reports duplicate op id(s)",
            requirement.id
        ));
    }
    check_duplicate_backend_conformance_pair_op_ids(requirement, suffix, &report, failures);
    if let (Some(expected), Some(pairs)) = (
        expected_backend,
        report.get("pairs").and_then(serde_json::Value::as_array),
    ) {
        for pair in pairs {
            let op_id = pair
                .get("op_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            let backend_id = pair.get("backend_id").and_then(serde_json::Value::as_str);
            if backend_id != Some(expected) {
                failures.push(format!(
                    "requirement `{}` backend conformance `{suffix}` pair `{op_id}` reports backend `{:?}`, expected `{expected}`",
                    requirement.id,
                    backend_id
                ));
            }
        }
    }
}

fn check_duplicate_backend_conformance_pair_op_ids(
    requirement: &Requirement,
    suffix: &str,
    report: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            report, "pairs", "op_id",
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        failures.push(format!(
            "requirement `{}` backend conformance `{suffix}` has duplicate pair op_id rows: {duplicates}",
            requirement.id
        ));
    }
}

#[cfg(test)]
mod part5_tests {
    use super::*;

    #[test]
    fn parser_contract_rejects_duplicate_required_object_rows() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for parser contract duplicate row test.");
        let report = serde_json::json!({
            "component_id": "vyrec",
            "role": "cli parser contract",
            "root": "crates/vyrec",
            "required_terms": ["parse"],
            "missing_terms": [],
            "required_contract_topics": ["ownership"],
            "missing_contract_topics": [],
            "required_test_categories": ["unit"],
            "missing_test_categories": [],
            "required_evidence_trees": [
                {"tree": "tests", "exists": true, "source_bytes": 128, "unreadable_file_count": 0},
                {"tree": "tests", "exists": true, "source_bytes": 128, "unreadable_file_count": 0},
                {"tree": "benches", "exists": true, "source_bytes": 128, "unreadable_file_count": 0}
            ],
            "unresolved_ownership_markers": [],
            "required_files": [
                {"path": "crates/vyrec/src/lib.rs", "exists": true, "source_bytes": 128, "read_error": null},
                {"path": "crates/vyrec/src/lib.rs", "exists": true, "source_bytes": 128, "read_error": null}
            ]
        });
        std::fs::write(
            dir.path().join("vyrec-cli-contracts.json"),
            report.to_string(),
        )
        .expect("Fix: write parser contract duplicate row fixture.");
        let requirement = Requirement {
            id: "parser-contract".to_string(),
            title: "parser contract".to_string(),
            status: "required".to_string(),
            evidence: vec!["vyrec-cli-contracts.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_parser_contract_evidence(
            &requirement,
            dir.path(),
            "vyrec-cli-contracts.json",
            &mut failures,
        );

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicate required_evidence_trees.tree rows: tests")),
            "Fix: parser contract gate must reject duplicate required evidence tree rows; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "duplicate required_files.path rows: crates/vyrec/src/lib.rs"
            )),
            "Fix: parser contract gate must reject duplicate required file rows; failures={failures:?}"
        );
    }

    #[test]
    fn backend_conformance_rejects_duplicate_pair_op_ids() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for backend conformance duplicate pair test.");
        let report = serde_json::json!({
            "schema_version": 2,
            "backend_id": "cuda",
            "total_pairs": 49,
            "failed_pairs": 0,
            "distinct_op_count": 49,
            "catalog_required_op_count": 49,
            "catalog_covered_op_count": 49,
            "missing_catalog_ops": [],
            "op_matrix_blocked_release_count": 0,
            "release_backend_row_count": 147,
            "missing_release_backend_rows": [],
            "op_matrix_errors": [],
            "duplicate_op_ids": [],
            "pairs": [
                {"op_id": "vyre.add", "backend_id": "cuda"},
                {"op_id": "vyre.add", "backend_id": "cuda"}
            ]
        });
        std::fs::write(dir.path().join("cuda-conformance.json"), report.to_string())
            .expect("Fix: write backend conformance duplicate pair fixture.");
        let requirement = Requirement {
            id: "conformance-hard-gate".to_string(),
            title: "conformance".to_string(),
            status: "required".to_string(),
            evidence: vec!["cuda-conformance.json".to_string()],
            minimum_evidence: 1,
        };
        let mut failures = Vec::new();

        check_backend_conformance_report(
            &requirement,
            dir.path(),
            "cuda-conformance.json",
            &mut failures,
        );

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicate pair op_id rows: vyre.add")),
            "Fix: backend conformance gate must reject duplicate pairs[].op_id even when duplicate_op_ids claims clean evidence; failures={failures:?}"
        );
    }
}

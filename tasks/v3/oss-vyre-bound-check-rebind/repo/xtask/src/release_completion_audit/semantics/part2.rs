fn inspect_weir_matrix_semantics(
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
            "{evidence}: schema_version is {schema_version}, expected >= 2"
        ));
    }
    let inventory_registered = value
        .get("inventory_registered_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if inventory_registered == 0 {
        blockers.push(format!(
            "{evidence}: inventory_registered_count must be nonzero"
        ));
    }
    let required_api_item_count = value
        .get("required_api_item_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if required_api_item_count < 100 {
        blockers.push(format!(
            "{evidence}: required_api_item_count is {required_api_item_count}; release matrix must prove at least 100 named Weir public API items"
        ));
    }
    let missing_api_item_count = value
        .get("missing_api_item_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    if missing_api_item_count != 0 {
        blockers.push(format!(
            "{evidence}: missing_api_item_count is {missing_api_item_count}; release requires zero missing Weir public API items"
        ));
    }
    for (field, label, minimum) in [
        ("property_test_count", "property", 15_u64),
        ("parity_test_count", "parity", 4_u64),
        ("adversarial_test_count", "adversarial", 1_u64),
        ("perf_test_count", "perf/scale", 2_u64),
        ("fuzz_test_count", "fuzz", 1_u64),
        ("gap_test_count", "gap", 1_u64),
    ] {
        let count = value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if count < minimum {
            blockers.push(format!(
                "{evidence}: {label} test family count is {count}; needs at least {minimum}"
            ));
        }
    }
    let standalone_examples = value
        .get("standalone_example_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_examples < 2 {
        blockers.push(format!(
            "{evidence}: standalone_example_count is {standalone_examples}; needs at least 2 examples outside tests"
        ));
    }
    let standalone_serde_evidence = value
        .get("standalone_serde_evidence_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_serde_evidence == 0 {
        blockers.push(format!(
            "{evidence}: standalone_serde_evidence_count must be nonzero"
        ));
    }
    let standalone_serde_feature_guards = value
        .get("standalone_serde_feature_guard_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_serde_feature_guards == 0 {
        blockers.push(format!(
            "{evidence}: standalone_serde_feature_guard_count must prove required-features = [\"serde\"] for serde evidence examples"
        ));
    }
    let example_files = value
        .get("standalone_examples")
        .and_then(serde_json::Value::as_array);
    if example_files.is_none_or(|examples| examples.len() < 2) {
        blockers.push(format!(
            "{evidence}: standalone_examples must list at least 2 example files"
        ));
    }
    let standalone_example_scan_errors = value
        .get("standalone_example_scan_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if standalone_example_scan_errors != 0 {
        blockers.push(format!(
            "{evidence}: reports {standalone_example_scan_errors} standalone example scan error(s)"
        ));
    }
    if let Some(examples) = example_files {
        for example in examples {
            let path = example
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if example.get("exists").and_then(serde_json::Value::as_bool) != Some(true)
                || example
                    .get("source_bytes")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
            {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` must exist and be non-empty"
                ));
            }
            if !example
                .get("read_error")
                .is_some_and(serde_json::Value::is_null)
            {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` read_error must be null"
                ));
            }
            if example.get("has_main").and_then(serde_json::Value::as_bool) != Some(true) {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` must expose runnable fn main"
                ));
            }
            if example
                .get("uses_weir_crate")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` must import or reference the weir crate"
                ));
            }
            let api_reference_count = example
                .get("api_reference_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if api_reference_count < 2 {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` references {api_reference_count} dataflow API token(s); needs at least 2"
                ));
            }
            if path.ends_with("serde_evidence.rs")
                && example
                    .get("has_serde_evidence")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                blockers.push(format!(
                    "{evidence}: standalone serde example `{path}` must report has_serde_evidence=true"
                ));
            }
            let unresolved_markers = example
                .get("unresolved_markers")
                .and_then(serde_json::Value::as_array)
                .map_or(usize::MAX, Vec::len);
            if unresolved_markers != 0 {
                blockers.push(format!(
                    "{evidence}: standalone example `{path}` reports {unresolved_markers} unresolved marker(s)"
                ));
            }
        }
    }
    let untested_analyses = value
        .get("untested_analyses")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if untested_analyses != 0 {
        blockers.push(format!(
            "{evidence}: {untested_analyses} Weir analysis module(s) lack release test coverage"
        ));
    }
    let Some(analyses) = value.get("analyses").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing analyses array"));
        return;
    };
    if analyses.is_empty() {
        blockers.push(format!("{evidence}: analyses array is empty"));
    }
    for entry in analyses {
        let id = entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        let declares_op_id = entry
            .get("declares_op_id")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let registered = entry
            .get("inventory_registered")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let required_api_items = entry
            .get("required_api_items")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        let missing_api_items = entry
            .get("missing_api_items")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if required_api_items != 0 && missing_api_items != 0 {
            blockers.push(format!(
                "{evidence}: analysis `{id}` reports {missing_api_items} missing required API item(s)"
            ));
        }
        if id == "soundness" {
            let required = entry
                .get("required_policy_items")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            let missing = entry
                .get("missing_policy_items")
                .and_then(serde_json::Value::as_array)
                .map_or(usize::MAX, Vec::len);
            if required < 6 || missing != 0 {
                blockers.push(format!(
                    "{evidence}: soundness analysis must prove six policy API items and report zero missing items"
                ));
            }
        }
        if declares_op_id && !registered {
            blockers.push(format!(
                "{evidence}: analysis `{id}` declares OP_ID without inventory registration"
            ));
        }
    }
}

fn inspect_backend_conformance_semantics(
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
            "{evidence}: schema_version={schema_version}; backend conformance evidence must be schema>=2"
        ));
    }
    let expected_backend = if evidence.ends_with("cuda-conformance.json") {
        "cuda"
    } else if evidence.ends_with("wgpu-conformance.json") {
        "wgpu"
    } else {
        "cpu-ref"
    };
    if value.get("backend_id").and_then(serde_json::Value::as_str) != Some(expected_backend) {
        blockers.push(format!(
            "{evidence}: backend_id must be `{expected_backend}`"
        ));
    }
    let total_pairs = value
        .get("total_pairs")
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
    let release_backend_row_count = value
        .get("release_backend_row_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let missing_release_backend_rows = value
        .get("missing_release_backend_rows")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let op_matrix_errors = value
        .get("op_matrix_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if op_matrix_errors != 0 {
        blockers.push(format!(
            "{evidence}: reports {op_matrix_errors} OP_MATRIX read/parse error(s)"
        ));
    }
    if total_pairs < 49 {
        blockers.push(format!(
            "{evidence}: total_pairs is {total_pairs}, below release floor 49"
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
    let expected_release_backend_rows = catalog_required_op_count.saturating_mul(3);
    if release_backend_row_count < expected_release_backend_rows
        || missing_release_backend_rows != 0
    {
        blockers.push(format!(
            "{evidence}: release_backend_row_count={release_backend_row_count}, expected {expected_release_backend_rows}, missing_release_backend_rows={missing_release_backend_rows}"
        ));
    }
    if value
        .get("failed_pairs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX)
        != 0
    {
        blockers.push(format!("{evidence}: failed_pairs must be zero"));
    }
    if value
        .get("duplicate_op_ids")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|duplicates| !duplicates.is_empty())
    {
        blockers.push(format!("{evidence}: duplicate_op_ids must be empty"));
    }
    inspect_duplicate_backend_conformance_pair_op_ids(evidence, value, blockers);
}

fn inspect_duplicate_backend_conformance_pair_op_ids(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            value, "pairs", "op_id",
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!(
            "{evidence}: duplicate backend conformance pair op_id rows: {duplicates}"
        ));
    }
}

#[cfg(test)]
mod part2_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_duplicate_backend_conformance_pair_op_ids() {
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
        let mut blockers = Vec::new();

        inspect_backend_conformance_semantics("cuda-conformance.json", &report, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker
                .contains("duplicate backend conformance pair op_id rows: vyre.add")),
            "Fix: completion audit must reject duplicate backend conformance pairs[].op_id even when duplicate_op_ids claims clean evidence; blockers={blockers:?}"
        );
    }
}

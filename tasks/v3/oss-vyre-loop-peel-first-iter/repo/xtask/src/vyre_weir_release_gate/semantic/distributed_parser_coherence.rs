use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(
        requirement,
        base_dir,
        "distributed-parser-map.json",
        failures,
    ) else {
        return;
    };
    let components = matrix
        .get("components")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if components == 0 {
        failures.push(
            "requirement `distributed-parser-coherence` matrix contains zero components"
                .to_string(),
        );
    }
    let component_ids = matrix
        .get("components")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in [
        "vyre-frontend-c",
        "vyrec",
        "weir",
        "surgec",
        "surgec-grammar-gen",
    ] {
        if !component_ids.iter().any(|component| {
            component.get("id").and_then(serde_json::Value::as_str) == Some(required)
                && component.get("exists").and_then(serde_json::Value::as_bool) == Some(true)
                && component
                    .get("missing_terms")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("missing_contract_topics")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("required_test_categories")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|categories| !categories.is_empty())
                && component
                    .get("missing_test_categories")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("unresolved_ownership_markers")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("required_files")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|files| {
                        !files.is_empty()
                            && files.iter().all(|file| {
                                file.get("exists").and_then(serde_json::Value::as_bool)
                                    == Some(true)
                                    && file
                                        .get("source_bytes")
                                        .and_then(serde_json::Value::as_u64)
                                        .unwrap_or(0)
                                        > 0
                                    && file
                                        .get("read_error")
                                        .is_some_and(serde_json::Value::is_null)
                            })
                    })
        }) {
            failures.push(format!(
                "requirement `distributed-parser-coherence` matrix is missing complete component `{required}`"
            ));
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `distributed-parser-coherence` matrix still reports {blockers} blocker(s)"
        ));
    }
    for suffix in [
        "vyre-frontend-c-contracts.json",
        "vyrec-cli-contracts.json",
        "weir-contracts.json",
        "surgec-contracts.json",
        "surgec-grammar-gen-contracts.json",
    ] {
        check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
        check_parser_contract_evidence(requirement, base_dir, suffix, failures);
    }
}

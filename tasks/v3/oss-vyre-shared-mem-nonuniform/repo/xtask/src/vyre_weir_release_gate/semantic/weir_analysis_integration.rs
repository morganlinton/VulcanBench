use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(
        requirement,
        base_dir,
        "weir-analysis-api-matrix.json",
        failures,
    ) else {
        return;
    };
    let analyses = matrix
        .get("analyses")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let schema_version = matrix
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        failures.push(format!(
            "requirement `weir-analysis-integration` matrix schema_version is {schema_version}, expected >= 2"
        ));
    }
    if analyses == 0 {
        failures.push(
            "requirement `weir-analysis-integration` matrix contains zero analyses".to_string(),
        );
    }
    let inventory_registered = matrix
        .get("inventory_registered_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if inventory_registered == 0 {
        failures.push(
            "requirement `weir-analysis-integration` matrix contains zero inventory-registered analyses"
                .to_string(),
        );
    }
    let required_api_item_count = matrix
        .get("required_api_item_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if required_api_item_count < 100 {
        failures.push(format!(
            "requirement `weir-analysis-integration` Weir matrix proves {required_api_item_count} required API item(s), needs at least 100"
        ));
    }
    let missing_api_item_count = matrix
        .get("missing_api_item_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    if missing_api_item_count != 0 {
        failures.push(format!(
            "requirement `weir-analysis-integration` Weir matrix reports {missing_api_item_count} missing required API item(s)"
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
        let count = matrix
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if count < minimum {
            failures.push(format!(
                "requirement `weir-analysis-integration` matrix contains {count} {label} test families; needs at least {minimum}"
            ));
        }
    }
    let standalone_examples = matrix
        .get("standalone_example_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_examples < 2 {
        failures.push(format!(
            "requirement `weir-analysis-integration` matrix contains {standalone_examples} standalone example(s); needs at least 2 examples outside tests"
        ));
    }
    let standalone_serde_evidence = matrix
        .get("standalone_serde_evidence_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_serde_evidence == 0 {
        failures.push(
            "requirement `weir-analysis-integration` matrix must include at least one standalone serde evidence example for witness/soundness API records"
                .to_string(),
        );
    }
    let standalone_serde_feature_guards = matrix
        .get("standalone_serde_feature_guard_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if standalone_serde_feature_guards == 0 {
        failures.push(
            "requirement `weir-analysis-integration` matrix must prove serde evidence examples declare required-features = [\"serde\"]"
                .to_string(),
        );
    }
    if matrix
        .get("standalone_examples")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|examples| examples.len() < 2)
    {
        failures.push(
            "requirement `weir-analysis-integration` matrix must list at least 2 standalone example files"
                .to_string(),
        );
    }
    let standalone_example_scan_errors = matrix
        .get("standalone_example_scan_errors")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if standalone_example_scan_errors != 0 {
        failures.push(format!(
            "requirement `weir-analysis-integration` matrix reports {standalone_example_scan_errors} standalone example scan error(s)"
        ));
    }
    if let Some(examples) = matrix
        .get("standalone_examples")
        .and_then(serde_json::Value::as_array)
    {
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
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` must exist and be non-empty"
                ));
            }
            if !example
                .get("read_error")
                .is_some_and(serde_json::Value::is_null)
            {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` read_error must be null"
                ));
            }
            if example.get("has_main").and_then(serde_json::Value::as_bool) != Some(true) {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` must expose runnable fn main"
                ));
            }
            if example
                .get("uses_weir_crate")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` must import or reference the weir crate"
                ));
            }
            let api_reference_count = example
                .get("api_reference_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if api_reference_count < 2 {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` references {api_reference_count} dataflow API token(s); needs at least 2"
                ));
            }
            if path.ends_with("serde_evidence.rs")
                && example
                    .get("has_serde_evidence")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone serde example `{path}` must report has_serde_evidence=true"
                ));
            }
            let unresolved_markers = example
                .get("unresolved_markers")
                .and_then(serde_json::Value::as_array)
                .map_or(usize::MAX, Vec::len);
            if unresolved_markers != 0 {
                failures.push(format!(
                    "requirement `weir-analysis-integration` standalone example `{path}` reports {unresolved_markers} unresolved marker(s)"
                ));
            }
        }
    }
    let untested_analyses = matrix
        .get("untested_analyses")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if untested_analyses != 0 {
        failures.push(format!(
            "requirement `weir-analysis-integration` matrix reports {untested_analyses} Weir analysis module(s) without release test coverage"
        ));
    }
    if let Some(entries) = matrix.get("analyses").and_then(serde_json::Value::as_array) {
        for entry in entries {
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
                failures.push(format!(
                    "requirement `weir-analysis-integration` analysis `{id}` reports {missing_api_items} missing required API item(s)"
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
                    failures.push(
                        "requirement `weir-analysis-integration` soundness analysis must prove six policy API items and report zero missing items"
                            .to_string(),
                    );
                }
            }
            if declares_op_id && !registered {
                failures.push(format!(
                    "requirement `weir-analysis-integration` analysis `{id}` declares OP_ID without inventory registration"
                ));
            }
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `weir-analysis-integration` matrix still reports {blockers} blocker(s)"
        ));
    }
    check_json_evidence_has_no_blockers(
        requirement,
        base_dir,
        "weir-vyre-integration-tests.json",
        failures,
    );
    if let Some(integration) = first_json_evidence(
        requirement,
        base_dir,
        "weir-vyre-integration-tests.json",
        failures,
    ) {
        let schema_version = integration
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if schema_version < 2 {
            failures.push(format!(
                "requirement `weir-analysis-integration` Weir integration evidence schema_version is {schema_version}, expected >= 2"
            ));
        }
    }
    if let Some(readme) = first_json_evidence(
        requirement,
        base_dir,
        "weir-readme-contracts.json",
        failures,
    ) {
        let schema_version = readme
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if schema_version < 2 {
            failures.push(format!(
                "requirement `weir-analysis-integration` Weir README contract schema_version is {schema_version}, expected >= 2"
            ));
        }
        if readme.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
            failures.push(
                "requirement `weir-analysis-integration` Weir README contract does not prove README.md exists"
                    .to_string(),
            );
        }
        if readme
            .get("source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(
                "requirement `weir-analysis-integration` Weir README contract reports empty README.md"
                    .to_string(),
            );
        }
        if readme
            .get("missing_tokens")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|tokens| !tokens.is_empty())
        {
            failures.push(
                "requirement `weir-analysis-integration` Weir README is missing required API/version tokens"
                    .to_string(),
            );
        }
        if readme
            .get("example_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(
                "requirement `weir-analysis-integration` Weir README has no Rust/TOML example block"
                    .to_string(),
            );
        }
        let blockers = readme
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if blockers != 0 {
            failures.push(format!(
                "requirement `weir-analysis-integration` Weir README contract reports {blockers} blocker(s)"
            ));
        }
    }
    check_marker_evidence_has_markers(
        requirement,
        base_dir,
        "weir-facts-pass-firing.json",
        failures,
    );
    check_named_cuda_benchmark_report(
        requirement,
        base_dir,
        "dataflow-analysis-release.json",
        failures,
    );
}

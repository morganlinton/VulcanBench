use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(requirement, base_dir, "test-matrix.json", failures)
    else {
        return;
    };
    let test_files = matrix
        .get("test_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if test_files == 0 {
        failures.push(format!(
            "requirement `{}` test matrix contains zero test files",
            requirement.id
        ));
    }
    for (field, label) in [
        ("vyre_test_files", "Vyre"),
        ("weir_test_files", "Weir"),
        ("vyrec_test_files", "tools/vyrec"),
    ] {
        if matrix
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            failures.push(format!(
                "requirement `{}` test matrix contains zero {label} release-surface test files",
                requirement.id
            ));
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `{}` test matrix still reports {blockers} blocker(s)",
            requirement.id
        ));
    }
    let layers = matrix
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in [
        "unit",
        "integration",
        "property",
        "adversarial",
        "corpus",
        "benchmark",
        "conformance",
        "gap",
        "fuzz",
    ] {
        if !layers.iter().any(|layer| layer.as_str() == Some(required)) {
            failures.push(format!(
                "requirement `{}` test matrix is missing `{required}` layer evidence",
                requirement.id
            ));
        }
    }
    if !matrix
        .get("oversized_files")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        failures.push(format!(
            "requirement `{}` test matrix still contains oversized test files",
            requirement.id
        ));
    }
    if !matrix
        .get("god_test_candidates")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        failures.push(format!(
            "requirement `{}` test matrix still contains monolithic tests.rs candidates",
            requirement.id
        ));
    }
    check_release_surface_coverage(requirement, &matrix, failures);
    match requirement.id.as_str() {
        "modular-test-architecture" => {
            for suffix in [
                "modularization-map.json",
                "oversized-test-closure.json",
                "release-surface-suite-coverage.json",
            ] {
                check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
            }
            if let Some(modularization) =
                first_json_evidence(requirement, base_dir, "modularization-map.json", failures)
            {
                let directories = modularization
                    .get("directories")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for required_surface in ["vyre", "weir", "vyrec"] {
                    if !directories.iter().any(|directory| {
                        directory.get("surface").and_then(serde_json::Value::as_str)
                            == Some(required_surface)
                    }) {
                        failures.push(format!(
                            "requirement `modular-test-architecture` modularization map is missing `{required_surface}` surface directories"
                        ));
                    }
                }
            }
            if let Some(closure) = first_json_evidence(
                requirement,
                base_dir,
                "oversized-test-closure.json",
                failures,
            ) {
                if closure.get("closed").and_then(serde_json::Value::as_bool) != Some(true) {
                    failures.push(
                        "requirement `modular-test-architecture` oversized-test closure is not closed"
                            .to_string(),
                    );
                }
                if closure
                    .get("total_oversized_files")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(u64::MAX)
                    != 0
                {
                    failures.push(
                        "requirement `modular-test-architecture` oversized-test closure still has oversized files"
                            .to_string(),
                    );
                }
                if closure
                    .get("total_god_test_candidates")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(u64::MAX)
                    != 0
                {
                    failures.push(
                        "requirement `modular-test-architecture` oversized-test closure still has monolithic tests.rs files"
                            .to_string(),
                    );
                }
            }
        }
        "exhaustive-verification" => {
            for suffix in [
                "unit-suite.json",
                "adversarial-suite.json",
                "property-suite.json",
                "conformance-suite.json",
                "corpus-suite.json",
                "benchmark-suite.json",
                "gap-suite.json",
                "fuzz-suite.json",
            ] {
                check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
                if let Some(suite) = first_json_evidence(requirement, base_dir, suffix, failures) {
                    if suite
                        .get("file_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        failures.push(format!(
                            "requirement `exhaustive-verification` suite `{suffix}` has zero files"
                        ));
                    }
                    if suite
                        .get("vyre_file_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        failures.push(format!(
                            "requirement `exhaustive-verification` suite `{suffix}` has zero Vyre-side files"
                        ));
                    }
                    if suite
                        .get("dataflow_consumer_file_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        failures.push(format!(
                            "requirement `exhaustive-verification` suite `{suffix}` has zero Weir-side files"
                        ));
                    }
                    if suite
                        .get("vyrec_file_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        failures.push(format!(
                            "requirement `exhaustive-verification` suite `{suffix}` has zero tools/vyrec-side files"
                        ));
                    }
                }
            }
            check_json_evidence_has_no_blockers(
                requirement,
                base_dir,
                "release-surface-suite-coverage.json",
                failures,
            );
        }
        _ => {}
    }
}

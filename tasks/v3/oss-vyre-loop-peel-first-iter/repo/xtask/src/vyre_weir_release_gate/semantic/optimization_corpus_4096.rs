use std::path::Path;

use crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(corpus) =
        first_json_evidence(requirement, base_dir, "optimization-corpus.json", failures)
    else {
        return;
    };
    let generated = corpus
        .get("generated_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let verified = corpus
        .get("verified_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let optimized = corpus
        .get("optimized_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dataflow_analysis_cases = corpus
        .get("dataflow_analysis_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dataflow_analysis_optimized = corpus
        .get("dataflow_analysis_optimized_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let non_converged = corpus
        .get("non_converged_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let blockers = corpus
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let required = corpus
        .get("required_min_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(4_096);
    if required < 4_096 {
        failures.push(format!(
            "requirement `optimization-corpus-4096` required_min_cases={required}; release floor is 4096"
        ));
    }
    if generated < required || generated < 4_096 {
        failures.push(format!(
            "requirement `optimization-corpus-4096` generated {generated} cases; needs at least {required} and never below 4096"
        ));
    }
    if verified != generated {
        failures.push(format!(
            "requirement `optimization-corpus-4096` verified {verified}/{generated} generated cases through verify_then_optimize"
        ));
    }
    if optimized == 0 {
        failures.push(
            "requirement `optimization-corpus-4096` reports zero optimized cases; corpus is not proving rewrite coverage"
                .to_string(),
        );
    }
    if dataflow_analysis_cases == 0 {
        failures.push(
            "requirement `optimization-corpus-4096` reports zero dataflow-analysis-aware cases"
                .to_string(),
        );
    }
    if dataflow_analysis_optimized < dataflow_analysis_cases {
        failures.push(format!(
            "requirement `optimization-corpus-4096` optimized {dataflow_analysis_optimized}/{dataflow_analysis_cases} dataflow-analysis-aware cases"
        ));
    }
    if non_converged != 0 || blockers != 0 {
        failures.push(format!(
            "requirement `optimization-corpus-4096` reports {non_converged} non-converged case(s) and {blockers} blocker(s)"
        ));
    }
    for suffix in [
        "optimization-corpus-contracts.json",
        "optimization-family-manifest.json",
        "optimization-analysis-fixtures.json",
        "optimization-case-manifest.json",
    ] {
        check_json_evidence_has_no_blockers(requirement, base_dir, suffix, failures);
    }
    if let Some(family_manifest) = first_json_evidence(
        requirement,
        base_dir,
        "optimization-family-manifest.json",
        failures,
    ) {
        let families = family_manifest
            .get("families")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if families.len() < 14 {
            failures.push(format!(
                "requirement `optimization-corpus-4096` family manifest lists {} optimization families; needs at least 14 required release families",
                families.len()
            ));
        }
        let declared_required = family_manifest
            .get("required_family_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if declared_required < 14 {
            failures.push(format!(
                "requirement `optimization-corpus-4096` family manifest declares {declared_required} required optimization families; needs all 14 release families"
            ));
        }
        let missing_required = family_manifest
            .get("missing_required_families")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if missing_required != 0 {
            failures.push(format!(
                "requirement `optimization-corpus-4096` family manifest reports {missing_required} missing required optimization family/families"
            ));
        }
        check_duplicate_optimization_family_rows(&family_manifest, failures);
        for required in [
            "algebraic",
            "predicate",
            "egraph",
            "memory-layout",
            "control-flow",
            "vector-layout",
            "A13-coalesce-fixture",
            "A14-shared-mem-promote-fixture",
            "A15-bank-conflict-fixture",
            "A16-vec-pack-fixture",
            "weir-dataflow-dse",
            "weir-dataflow-loop-fusion",
            "weir-dataflow-loop-fission",
            "weir-dataflow-licm",
        ] {
            let required_cases = families
                .iter()
                .find(|family| {
                    family.get("family").and_then(serde_json::Value::as_str) == Some(required)
                })
                .and_then(|family| family.get("cases").and_then(serde_json::Value::as_u64))
                .unwrap_or(0);
            if required_cases < 128 {
                failures.push(format!(
                    "requirement `optimization-corpus-4096` required family `{required}` has {required_cases} generated case(s), needs at least 128"
                ));
            }
        }
        for family in &families {
            let name = family
                .get("family")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if family
                .get("family")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
                || family
                    .get("cases")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
            {
                failures.push(format!(
                    "requirement `optimization-corpus-4096` family manifest contains invalid family `{name}`"
                ));
            }
        }
    }
    if let Some(fixture_manifest) = first_json_evidence(
        requirement,
        base_dir,
        "optimization-analysis-fixtures.json",
        failures,
    ) {
        check_optimization_analysis_fixture_manifest(&fixture_manifest, failures);
    }
    if let Some(case_manifest) = first_json_evidence(
        requirement,
        base_dir,
        "optimization-case-manifest.json",
        failures,
    ) {
        let pass_instances = case_manifest
            .get("pass_instance_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let unique_case_ids = case_manifest
            .get("unique_case_ids")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let manifest_generated = case_manifest
            .get("generated_cases")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let entries = case_manifest
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if pass_instances != generated || manifest_generated != generated {
            failures.push(format!(
                "requirement `optimization-corpus-4096` case manifest pass_instance_count={pass_instances}, generated_cases={manifest_generated}, corpus generated_cases={generated}"
            ));
        }
        if pass_instances < 4_096 || unique_case_ids != pass_instances {
            failures.push(format!(
                "requirement `optimization-corpus-4096` case manifest has {pass_instances} pass instance(s) and {unique_case_ids} unique id(s); needs >=4096 unique pass instances"
            ));
        }
        if entries.len() as u64 != pass_instances {
            failures.push(format!(
                "requirement `optimization-corpus-4096` case manifest lists {} entrie(s), pass_instance_count is {pass_instances}",
                entries.len()
            ));
        }
        check_duplicate_optimization_case_entry_ids(&case_manifest, failures);
        for field in [
            "cases_with_child_bodies",
            "cases_with_bindings",
            "cases_with_literals",
        ] {
            if case_manifest
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `optimization-corpus-4096` case manifest `{field}` must be nonzero"
                ));
            }
        }
        let malformed_entries = entries
            .iter()
            .filter(|entry| {
                entry
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
                    || entry
                        .get("family")
                        .and_then(serde_json::Value::as_str)
                        .is_none_or(str::is_empty)
                    || entry
                        .get("total_ops")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
            })
            .count();
        if malformed_entries != 0 {
            failures.push(format!(
                "requirement `optimization-corpus-4096` case manifest contains {malformed_entries} malformed generated pass instance(s)"
            ));
        }
    }
}

fn check_duplicate_optimization_case_entry_ids(
    case_manifest: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let duplicates = duplicate_nonblank_object_array_field_values(case_manifest, "entries", "id");
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        failures.push(format!(
            "requirement `optimization-corpus-4096` case manifest has duplicate entry ids: {duplicates}"
        ));
    }
}

fn check_duplicate_optimization_family_rows(
    family_manifest: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let duplicates =
        duplicate_nonblank_object_array_field_values(family_manifest, "families", "family");
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        failures.push(format!(
            "requirement `optimization-corpus-4096` family manifest has duplicate family rows: {duplicates}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimization_corpus_gate_rejects_duplicate_family_rows() {
        let family_manifest = serde_json::json!({
            "required_family_count": 14,
            "missing_required_families": [],
            "families": [
                {"family": "algebraic", "cases": 128},
                {"family": "algebraic", "cases": 128}
            ],
            "blockers": []
        });
        let case_manifest = serde_json::json!({"blockers": []});
        let failures = run_optimization_corpus_gate_with_manifests(family_manifest, case_manifest);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("duplicate family rows: algebraic")),
            "Fix: optimization corpus gate must reject duplicate family manifest rows; failures={failures:?}"
        );
    }

    #[test]
    fn optimization_corpus_gate_rejects_duplicate_case_entry_ids() {
        let family_manifest = serde_json::json!({"blockers": []});
        let case_manifest = serde_json::json!({
            "pass_instance_count": 4096,
            "generated_cases": 4096,
            "unique_case_ids": 4096,
            "duplicate_case_ids": [],
            "entries": [
                {
                    "id": "optimization.algebraic.0001",
                    "family": "algebraic",
                    "total_ops": 3
                },
                {
                    "id": "optimization.algebraic.0001",
                    "family": "predicate",
                    "total_ops": 5
                }
            ],
            "cases_with_child_bodies": 1,
            "cases_with_bindings": 1,
            "cases_with_literals": 1,
            "blockers": []
        });
        let failures = run_optimization_corpus_gate_with_manifests(family_manifest, case_manifest);

        assert!(
            failures.iter().any(|failure| {
                failure.contains("duplicate entry ids: optimization.algebraic.0001")
            }),
            "Fix: optimization corpus gate must reject duplicate case manifest entry ids even when unique_case_ids and duplicate_case_ids claim clean evidence; failures={failures:?}"
        );
    }

    fn run_optimization_corpus_gate_with_manifests(
        family_manifest: serde_json::Value,
        case_manifest: serde_json::Value,
    ) -> Vec<String> {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temp workspace for optimization corpus gate test.");
        let base_dir = dir.path();
        std::fs::write(
            base_dir.join("optimization-corpus.json"),
            serde_json::json!({
                "required_min_cases": 4096,
                "generated_cases": 4096,
                "verified_cases": 4096,
                "optimized_cases": 4096,
                "dataflow_analysis_cases": 1,
                "dataflow_analysis_optimized_cases": 1,
                "non_converged_cases": 0,
                "blockers": []
            })
            .to_string(),
        )
        .expect("Fix: write optimization corpus gate fixture.");
        std::fs::write(
            base_dir.join("optimization-family-manifest.json"),
            family_manifest.to_string(),
        )
        .expect("Fix: write optimization family manifest fixture.");
        std::fs::write(
            base_dir.join("optimization-case-manifest.json"),
            case_manifest.to_string(),
        )
        .expect("Fix: write optimization case manifest fixture.");
        for suffix in [
            "optimization-corpus-contracts.json",
            "optimization-analysis-fixtures.json",
        ] {
            std::fs::write(
                base_dir.join(suffix),
                serde_json::json!({"blockers": []}).to_string(),
            )
            .expect("Fix: write auxiliary optimization corpus gate fixture.");
        }
        let requirement = Requirement {
            id: "optimization-corpus-4096".to_string(),
            title: "optimization corpus".to_string(),
            status: "required".to_string(),
            evidence: [
                "optimization-corpus.json",
                "optimization-corpus-contracts.json",
                "optimization-family-manifest.json",
                "optimization-analysis-fixtures.json",
                "optimization-case-manifest.json",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            minimum_evidence: 5,
        };
        let mut failures = Vec::new();

        check(&requirement, base_dir, &mut failures);

        failures
    }
}

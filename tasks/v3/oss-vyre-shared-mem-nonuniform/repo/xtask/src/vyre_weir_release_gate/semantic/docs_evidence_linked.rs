use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(requirement, base_dir, "docs-matrix.json", failures)
    else {
        return;
    };
    let docs = matrix
        .get("docs")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if docs == 0 {
        failures.push("requirement `docs-evidence-linked` matrix contains zero docs".to_string());
    }
    if matrix
        .get("curated_proof_docs_preserved")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        failures.push(
            "requirement `docs-evidence-linked` docs matrix must prove curated proof Markdown is create-if-missing and not overwritten"
                .to_string(),
        );
    }
    if matrix
        .get("limitation_findings")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|findings| !findings.is_empty())
    {
        failures.push(
            "requirement `docs-evidence-linked` matrix reports unapproved limitation or future-scope wording"
                .to_string(),
        );
    }
    if let Some(entries) = matrix.get("docs").and_then(serde_json::Value::as_array) {
        for entry in entries {
            let id = entry
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if entry
                .get("evidence_artifact_ref_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `docs-evidence-linked` doc `{id}` has zero concrete evidence artifact references"
                ));
            }
            if entry
                .get("missing_evidence_artifact_refs")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|refs| !refs.is_empty())
            {
                failures.push(format!(
                    "requirement `docs-evidence-linked` doc `{id}` references missing evidence artifacts"
                ));
            }
            if entry
                .get("missing_topics")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|topics| !topics.is_empty())
            {
                failures.push(format!(
                    "requirement `docs-evidence-linked` doc `{id}` has missing topics"
                ));
            }
            if entry
                .get("unresolved_markers")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|markers| !markers.is_empty())
            {
                failures.push(format!(
                    "requirement `docs-evidence-linked` doc `{id}` has unresolved markers"
                ));
            }
        }
    }
    if blockers != 0 {
        failures.push(format!(
            "requirement `docs-evidence-linked` matrix still reports {blockers} blocker(s)"
        ));
    }
    if let Some(readme) = first_json_evidence(
        requirement,
        base_dir,
        "vyre-readme-contracts.json",
        failures,
    ) {
        check_readme_contract("docs-evidence-linked", "Vyre", &readme, failures);
    }
    for suffix in [
        "vyre-readme-proof.md",
        "weir-readme-proof.md",
        "parser-doc-proof.md",
        "benchmark-doc-proof.md",
        "conformance-doc-proof.md",
        "release-notes.md",
    ] {
        check_markdown_evidence_ready(requirement, base_dir, suffix, failures);
    }
}

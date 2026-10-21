use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    if !requirement.evidence.iter().any(|evidence| {
        evidence.contains("cargo_full")
            && evidence.contains("xtask")
            && evidence.contains("release-evidence")
    }) {
        failures.push(
            "requirement `final-completion-audit` must include the cargo_full release-evidence command as concrete evidence"
                .to_string(),
        );
    }
    if !requirement.evidence.iter().any(|evidence| {
        evidence.contains("cargo_full")
            && evidence.contains("xtask")
            && evidence.contains("release-completion-audit")
    }) {
        failures.push(
            "requirement `final-completion-audit` must include the cargo_full release-completion-audit command as concrete evidence"
                .to_string(),
        );
    }
    if !requirement.evidence.iter().any(|evidence| {
        evidence.contains("cargo_full")
            && evidence.contains("xtask")
            && evidence.contains("vyre-release-gate")
    }) {
        failures.push(
            "requirement `final-completion-audit` must include the cargo_full vyre-release-gate command as concrete evidence"
                .to_string(),
        );
    }

    let Some(audit) = first_json_evidence(requirement, base_dir, "completion-audit.json", failures)
    else {
        return;
    };
    let blockers = audit
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let open = audit
        .get("blocked_or_open_requirements")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    if blockers != 0 || open != 0 {
        failures.push(format!(
            "requirement `final-completion-audit` still reports {blockers} blocker(s) and {open} open requirement(s)"
        ));
    }

    let Some(run) =
        first_json_evidence(requirement, base_dir, "release-evidence-run.json", failures)
    else {
        return;
    };
    check_release_evidence_run(requirement, &run, failures);
}

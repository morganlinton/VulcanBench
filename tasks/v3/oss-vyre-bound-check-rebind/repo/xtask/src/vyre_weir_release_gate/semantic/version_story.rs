use std::path::Path;

use super::super::checks::*;
use super::super::types::Requirement;
use crate::release_train;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    if !requirement.evidence.iter().any(|evidence| {
        evidence.contains("cargo_full")
            && evidence.contains("version-matrix")
            && evidence.contains("release/evidence/version/version-matrix.json")
    }) {
        failures.push(
            "requirement `version-story` must include the cargo_full version-matrix evidence command"
                .to_string(),
        );
    }
    let Some(matrix) = first_json_evidence(requirement, base_dir, "version-matrix.json", failures)
    else {
        return;
    };
    let blockers = matrix
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if blockers != 0 {
        failures.push(format!(
            "requirement `version-story` matrix still reports {blockers} blocker(s)"
        ));
    }
    let vyre_release = matrix
        .get("requested_vyre_release")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if vyre_release != release_train::vyre_version() {
        failures.push(format!(
            "requirement `version-story` requested_vyre_release is `{vyre_release}`, expected `{}`",
            release_train::vyre_version()
        ));
    }
    let weir_release = matrix
        .get("requested_weir_release")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if weir_release != release_train::weir_version() {
        failures.push(format!(
            "requirement `version-story` requested_weir_release is `{weir_release}`, expected `{}`",
            release_train::weir_version()
        ));
    }
    if matrix
        .get("release_doc_tag_findings")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|findings| !findings.is_empty())
    {
        failures.push(
            "requirement `version-story` release docs must not contain bare v0.6.3 tag commands"
                .to_string(),
        );
    }
    if matrix
        .get("release_note_token_findings")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|findings| !findings.is_empty())
    {
        failures.push(
            "requirement `version-story` release-note docs must include every required version and product-scoped tag token"
                .to_string(),
        );
    }
    if matrix
        .get("missing_required_release_packages")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|packages| !packages.is_empty())
    {
        failures.push(
            "requirement `version-story` missing_required_release_packages must be empty"
                .to_string(),
        );
    }
    let required_release_packages = matrix
        .get("required_release_packages")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required_package in [
        "vyre@0.6.3",
        "vyre-driver-cuda@0.6.3",
        "vyre-driver-wgpu@0.6.3",
        "weir@0.1.0",
        "vyrec@0.1.0",
        "vyre-frontend-c@0.6.3",
    ] {
        if !required_release_packages
            .iter()
            .any(|package| package.as_str() == Some(required_package))
        {
            failures.push(format!(
                "requirement `version-story` required_release_packages must include `{required_package}`"
            ));
        }
    }
    let Some(tag_story) = matrix
        .get("tag_story")
        .and_then(serde_json::Value::as_object)
    else {
        failures
            .push("requirement `version-story` version matrix is missing `tag_story`".to_string());
        return;
    };
    for (field, expected) in release_train::tag_story_fields() {
        let actual = tag_story
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if actual != expected {
            failures.push(format!(
                "requirement `version-story` tag_story.{field} is `{actual}`, expected `{expected}`"
            ));
        }
    }
    for required in release_train::required_release_note_tokens() {
        let present = tag_story
            .get("required_in_release_notes")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|entries| entries.iter().any(|entry| entry.as_str() == Some(required)));
        if !present {
            failures.push(format!(
                "requirement `version-story` tag_story.required_in_release_notes is missing `{required}`"
            ));
        }
    }
    if let Some(tag_plan) =
        first_json_evidence(requirement, base_dir, "release-tag-plan.json", failures)
    {
        let tag_plan_blockers = tag_plan
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if tag_plan_blockers != 0 {
            failures.push(format!(
                "requirement `version-story` release-tag-plan reports {tag_plan_blockers} blocker(s)"
            ));
        }
        for (field, expected) in release_train::tag_story_fields() {
            if tag_plan.get(field).and_then(serde_json::Value::as_str) != Some(expected) {
                failures.push(format!(
                    "requirement `version-story` release-tag-plan {field} must be `{expected}`"
                ));
            }
        }
        if !tag_plan
            .get("required_gate_before_rc_tag")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|command| {
                command.contains("version-matrix")
                    && command.contains("release-completion-audit")
                    && command.contains("vyre-release-gate")
                    && command.contains("scripts/apply-branch-protection.sh")
                    && command.contains("cargo_full")
            })
        {
            failures.push(
            "requirement `version-story` release-tag-plan must require version matrix regeneration, completion audit, release gate, and branch-protection application before RC tag creation"
                .to_string(),
        );
        }
        let order = tag_plan
            .get("tag_creation_order")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ordered_tags = order
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        for (rc, final_tag) in release_train::rc_to_final_tags() {
            let rc_index = ordered_tags.iter().position(|tag| *tag == rc);
            let final_index = ordered_tags.iter().position(|tag| *tag == final_tag);
            if !matches!((rc_index, final_index), (Some(left), Some(right)) if left < right) {
                failures.push(format!(
                    "requirement `version-story` release-tag-plan must list `{rc}` before `{final_tag}`"
                ));
            }
        }
        if !tag_plan
            .get("required_gate_before_tag")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|command| {
                command.contains("version-matrix")
                    && command.contains("release-completion-audit")
                    && command.contains("vyre-release-gate")
                    && command.contains("scripts/apply-branch-protection.sh")
                    && command.contains("cargo_full")
            })
        {
            failures.push(
            "requirement `version-story` release-tag-plan must require version matrix regeneration, completion audit, release gate, and branch-protection application before tag creation"
                .to_string(),
        );
        }
        let version_blockers = tag_plan
            .get("version_matrix_blocker_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        if version_blockers != 0 {
            failures.push(format!(
                "requirement `version-story` release-tag-plan carries {version_blockers} version matrix blocker(s)"
            ));
        }
    }
}

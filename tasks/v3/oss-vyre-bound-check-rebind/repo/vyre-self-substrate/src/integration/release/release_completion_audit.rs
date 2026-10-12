//! Final completion audit evidence validation.

use std::collections::BTreeSet;

/// Validated release completion audit proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseCompletionAuditProof {
    /// Requirement count in the audit.
    pub total_requirements: u64,
    /// Prompt-to-artifact checklist item count.
    pub checklist_count: u64,
}

/// Release completion audit validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseCompletionAuditError {
    /// Required evidence is missing.
    MissingEvidence {
        /// Missing evidence field.
        evidence: &'static str,
    },
    /// Required numeric field is missing.
    MissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Numeric field missed a release threshold.
    ThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
    /// A specific plan requirement id is missing from the prompt-to-artifact checklist.
    MissingRequirementId {
        /// Required id.
        id: String,
    },
}

impl std::fmt::Display for ReleaseCompletionAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEvidence { evidence } => write!(
                f,
                "release completion audit is missing {evidence}. Fix: regenerate the completion audit against the active paradigm-shift plan before publishing."
            ),
            Self::MissingNumber { field } => write!(
                f,
                "release completion audit has no numeric {field}. Fix: record exact requirement and checklist counters."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "release completion audit {field}={observed} missed required {required}. Fix: close every explicit requirement before cargo_full publish or git push."
            ),
            Self::MissingRequirementId { id } => write!(
                f,
                "release completion audit is missing prompt-to-artifact checklist id `{id}`. Fix: map every active plan item r001..r100 to concrete evidence before publishing."
            ),
        }
    }
}

impl std::error::Error for ReleaseCompletionAuditError {}

/// Validate a final completion audit against the active paradigm-shift plan.
pub fn validate_release_completion_audit(
    audit: &str,
) -> Result<ReleaseCompletionAuditProof, ReleaseCompletionAuditError> {
    for (evidence, needle) in [
        ("schema version", "\"schema_version\": 1"),
        (
            "active objective",
            "\"objective\": \"Complete release/plans/paradigm-shift-100-concrete.md\"",
        ),
        (
            "active plan path",
            "\"plan_path\": \"release/plans/paradigm-shift-100-concrete.md\"",
        ),
        (
            "prompt-to-artifact checklist",
            "\"prompt_to_artifact_checklist\"",
        ),
        ("success criteria", "\"success_criteria\""),
        ("requirements", "\"requirements\""),
        ("closed requirements", "\"closed_requirements\""),
        (
            "zero blocked/open requirements",
            "\"blocked_or_open_requirements\": 0",
        ),
        ("zero blockers", "\"blockers\": []"),
        ("CUDA release path", "CUDA"),
        ("release validation matrix", "release_validation_matrix"),
        ("release checklist gate", "release-checklist-gate"),
        ("release deep review gate", "release-deep-review-gate"),
        ("release public API doctests", "release-public-api-doctests"),
        (
            "release test taxonomy coverage",
            "release-test-taxonomy-coverage",
        ),
        ("release gap findings", "release-gap-findings"),
        ("release GPU evidence", "release-gpu-evidence"),
        ("release launch sequence", "release-launch-sequence"),
        (
            "CUDA optimization release evidence",
            "optimization-release-evidence",
        ),
        (
            "CUDA benchmark baseline evidence",
            "release-benchmark-baselines",
        ),
        ("C parser Linux subsystem", "c-parser-linux-subsystem"),
        ("Dataflow consumer release evidence", "dataflow"),
        ("cargo_full publish final step", "cargo_full publish"),
        ("public repository action", "public"),
        ("release branch push", "git push origin release"),
        ("release tag push", "git push --tags"),
        ("launch receipts", "\"launch_receipts\""),
        ("cargo_full publish receipts", "\"cargo_publish_receipts\""),
        (
            "repository visibility receipt",
            "\"repository_visibility_receipt\"",
        ),
        (
            "release branch push receipt",
            "\"release_branch_push_receipt\"",
        ),
        ("tag push receipt", "\"tag_push_receipt\""),
        ("executed launch receipt status", "\"status\": \"executed\""),
    ] {
        audit_contains(audit, evidence, needle)?;
    }

    let total_requirements = number_field(audit, "total_requirements")?;
    let checklist_count = count_occurrences(audit, "\"requirement_id\"");
    let closed_status_count = count_occurrences(audit, "\"status\": \"closed\"");

    require_at_least("total_requirements", total_requirements, 100)?;
    require_at_least(
        "prompt_to_artifact_checklist requirement_id count",
        checklist_count,
        100,
    )?;
    require_at_least(
        "closed requirement status count",
        closed_status_count,
        total_requirements,
    )?;
    require_unique_requirement_ids(audit)?;

    Ok(ReleaseCompletionAuditProof {
        total_requirements,
        checklist_count,
    })
}

fn audit_contains(
    audit: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ReleaseCompletionAuditError> {
    if audit.contains(needle) {
        Ok(())
    } else {
        Err(ReleaseCompletionAuditError::MissingEvidence { evidence })
    }
}

fn require_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), ReleaseCompletionAuditError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ReleaseCompletionAuditError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn count_occurrences(haystack: &str, needle: &str) -> u64 {
    haystack.matches(needle).count() as u64
}

fn require_unique_requirement_ids(audit: &str) -> Result<(), ReleaseCompletionAuditError> {
    let mut seen = BTreeSet::new();
    for item in 1..=100 {
        let compact = format!("\"requirement_id\":\"r{item:03}\"");
        let spaced = format!("\"requirement_id\": \"r{item:03}\"");
        if audit.contains(&compact) || audit.contains(&spaced) {
            seen.insert(item);
        }
    }
    for item in 1..=100 {
        if !seen.contains(&item) {
            return Err(ReleaseCompletionAuditError::MissingRequirementId {
                id: format!("r{item:03}"),
            });
        }
    }
    Ok(())
}

fn number_field(audit: &str, field: &'static str) -> Result<u64, ReleaseCompletionAuditError> {
    let key = format!("\"{field}\"");
    let start = audit
        .find(&key)
        .ok_or(ReleaseCompletionAuditError::MissingNumber { field })?;
    let after_key = &audit[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(ReleaseCompletionAuditError::MissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(ReleaseCompletionAuditError::MissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| ReleaseCompletionAuditError::MissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_stale_committed_audit() {
        assert_eq!(
            validate_release_completion_audit(include_str!(
                "../../../../release/evidence/final/completion-audit.json"
            ))
            .expect_err("stale completion audit must not prove active plan completion"),
            ReleaseCompletionAuditError::MissingEvidence {
                evidence: "active objective",
            }
        );
    }

    #[test]
    fn completion_audit_accepts_active_plan_launch_audit() {
        let audit = active_audit(100, true);

        let proof = validate_release_completion_audit(&audit)
            .expect("Fix: active completion audit should pass");

        assert_eq!(proof.total_requirements, 100);
        assert_eq!(proof.checklist_count, 100);
    }

    #[test]
    fn completion_audit_rejects_missing_git_push() {
        let audit = format!(
            r#"{{
          "schema_version": 1,
          "objective": "Complete release/plans/paradigm-shift-100-concrete.md",
          "plan_path": "release/plans/paradigm-shift-100-concrete.md",
          "success_criteria": ["CUDA release path", "C parser Linux subsystem", "dataflow", "release_validation_matrix", "release-checklist-gate", "release-deep-review-gate", "release-public-api-doctests", "release-test-taxonomy-coverage", "release-gap-findings", "release-gpu-evidence", "release-launch-sequence", "optimization-release-evidence", "release-benchmark-baselines"],
          "prompt_to_artifact_checklist": [{}],
          "requirements": [{}],
          "closed_requirements": [{}],
          "blocked_or_open_requirements": 0,
          "total_requirements": 100,
          "blockers": [],
          "final_launch": ["cargo_full publish", "public"],
          "artifacts": ["c-parser-linux-subsystem"]
        }}"#,
            requirement_rows(),
            requirement_rows(),
            closed_rows()
        );

        assert_eq!(
            validate_release_completion_audit(&audit)
                .expect_err("audit without git push should fail"),
            ReleaseCompletionAuditError::MissingEvidence {
                evidence: "release branch push",
            }
        );
    }

    #[test]
    fn completion_audit_rejects_twenty_item_proxy_audit() {
        let audit = active_audit(20, true);

        assert_eq!(
            validate_release_completion_audit(&audit)
                .expect_err("20 requirement proxy audit must not satisfy 100-item plan"),
            ReleaseCompletionAuditError::ThresholdMiss {
                field: "total_requirements",
                observed: 20,
                required: 100,
            }
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_requirement_id_proxy() {
        let duplicated_requirements = (1..=100)
            .map(|_| r#"{"requirement_id":"r001"}"#)
            .collect::<Vec<_>>()
            .join(",");
        let audit = format!(
            r#"{{
          "schema_version": 1,
          "objective": "Complete release/plans/paradigm-shift-100-concrete.md",
          "plan_path": "release/plans/paradigm-shift-100-concrete.md",
          "success_criteria": ["CUDA release path", "C parser Linux subsystem", "dataflow", "release_validation_matrix", "release-checklist-gate", "release-deep-review-gate", "release-public-api-doctests", "release-test-taxonomy-coverage", "release-gap-findings", "release-gpu-evidence", "release-launch-sequence", "optimization-release-evidence", "release-benchmark-baselines"],
          "prompt_to_artifact_checklist": [{duplicated_requirements}],
          "requirements": [{duplicated_requirements}],
          "closed_requirements": [{}],
          "blocked_or_open_requirements": 0,
          "total_requirements": 100,
          "blockers": [],
          "final_launch": ["cargo_full publish", "public", "git push origin release", "git push --tags"],
          "launch_receipts": {{
            "cargo_publish_receipts": [{{"status": "executed"}}],
            "repository_visibility_receipt": {{"status": "executed"}},
            "release_branch_push_receipt": {{"status": "executed"}},
            "tag_push_receipt": {{"status": "executed"}}
          }},
          "artifacts": ["c-parser-linux-subsystem"]
        }}"#,
            closed_rows()
        );

        assert_eq!(
            validate_release_completion_audit(&audit)
                .expect_err("duplicated checklist ids must not satisfy the 100-item active plan"),
            ReleaseCompletionAuditError::MissingRequirementId {
                id: "r002".to_owned(),
            }
        );
    }

    #[test]
    fn completion_audit_rejects_missing_cuda_optimization_gate() {
        let audit = active_audit(100, false);

        assert_eq!(
            validate_release_completion_audit(&audit)
                .expect_err("completion audit must name CUDA optimization release evidence"),
            ReleaseCompletionAuditError::MissingEvidence {
                evidence: "CUDA optimization release evidence",
            }
        );
    }

    #[test]
    fn completion_audit_rejects_launch_plan_without_receipts() {
        let audit = format!(
            r#"{{
          "schema_version": 1,
          "objective": "Complete release/plans/paradigm-shift-100-concrete.md",
          "plan_path": "release/plans/paradigm-shift-100-concrete.md",
          "success_criteria": ["CUDA release path", "C parser Linux subsystem", "dataflow", "release_validation_matrix", "release-checklist-gate", "release-deep-review-gate", "release-public-api-doctests", "release-test-taxonomy-coverage", "release-gap-findings", "release-gpu-evidence", "release-launch-sequence", "optimization-release-evidence", "release-benchmark-baselines"],
          "prompt_to_artifact_checklist": [{}],
          "requirements": [{}],
          "closed_requirements": ["all"],
          "blocked_or_open_requirements": 0,
          "total_requirements": 100,
          "blockers": [],
          "final_launch": ["cargo_full publish", "public", "git push origin release", "git push --tags"],
          "artifacts": ["c-parser-linux-subsystem"]
        }}"#,
            requirement_rows(),
            closed_rows()
        );

        assert_eq!(
            validate_release_completion_audit(&audit)
                .expect_err("planned final launch is not executed final launch"),
            ReleaseCompletionAuditError::MissingEvidence {
                evidence: "launch receipts",
            }
        );
    }

    fn active_audit(total_requirements: u64, include_optimization_gate: bool) -> String {
        let optimization_gate = if include_optimization_gate {
            "\"optimization-release-evidence\","
        } else {
            ""
        };
        format!(
            r#"{{
          "schema_version": 1,
          "objective": "Complete release/plans/paradigm-shift-100-concrete.md",
          "plan_path": "release/plans/paradigm-shift-100-concrete.md",
          "success_criteria": ["CUDA release path", "C parser Linux subsystem", "dataflow", "release_validation_matrix", "release-checklist-gate", "release-deep-review-gate", "release-public-api-doctests", "release-test-taxonomy-coverage", "release-gap-findings", "release-gpu-evidence", "release-launch-sequence", {optimization_gate} "release-benchmark-baselines"],
          "prompt_to_artifact_checklist": [{}],
          "requirements": [{}],
          "closed_requirements": ["all"],
          "blocked_or_open_requirements": 0,
          "total_requirements": {total_requirements},
          "blockers": [],
          "final_launch": ["cargo_full publish", "public", "git push origin release", "git push --tags"],
          "launch_receipts": {{
            "cargo_publish_receipts": [{{"status": "executed"}}],
            "repository_visibility_receipt": {{"status": "executed"}},
            "release_branch_push_receipt": {{"status": "executed"}},
            "tag_push_receipt": {{"status": "executed"}}
          }},
          "artifacts": ["c-parser-linux-subsystem"]
        }}"#,
            requirement_rows(),
            closed_rows()
        )
    }

    fn requirement_rows() -> String {
        (1..=100)
            .map(|id| format!(r#"{{"requirement_id":"r{id:03}"}}"#))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn closed_rows() -> String {
        (1..=100)
            .map(|_| r#"{"status": "closed"}"#)
            .collect::<Vec<_>>()
            .join(",")
    }
}

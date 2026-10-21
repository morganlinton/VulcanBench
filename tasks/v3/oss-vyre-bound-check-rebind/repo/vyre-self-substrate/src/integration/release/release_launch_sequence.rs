//! Final release launch sequence validation.

/// One final launch step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseLaunchStep<'a> {
    /// Stable launch step id.
    pub id: &'a str,
    /// Exact command or externally verified action.
    pub command: &'a str,
    /// Whether the step is green.
    pub green: bool,
}

/// Validated release launch sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseLaunchSequenceProof {
    /// Step count.
    pub step_count: usize,
}

/// Validated release tag-plan artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseTagPlanProof {
    /// Number of ordered release tags.
    pub tag_count: usize,
}

/// Validated final launch receipt artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseLaunchReceiptProof {
    /// Required executed receipt count.
    pub receipt_count: usize,
}

/// Release launch sequence validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseLaunchSequenceError {
    /// Required step is missing or out of order.
    MissingOrOutOfOrder {
        /// Expected step id.
        expected: &'static str,
        /// Expected index.
        index: usize,
    },
    /// Step metadata is empty.
    EmptyMetadata {
        /// Step id.
        id: String,
        /// Field.
        field: &'static str,
    },
    /// Step is not green.
    StepNotGreen {
        /// Step id.
        id: String,
    },
    /// Required command pattern is absent.
    InvalidCommand {
        /// Step id.
        id: String,
        /// Command.
        command: String,
        /// Required command fragment.
        required_fragment: &'static str,
    },
    /// Release tag-plan artifact is missing required launch evidence.
    TagPlanMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Final launch receipt artifact is missing required execution evidence.
    ReceiptMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
}

impl std::fmt::Display for ReleaseLaunchSequenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOrOutOfOrder { expected, index } => write!(
                f,
                "release launch sequence missing `{expected}` at index {index}. Fix: run release gates before publish, then cargo_full publish, then public repo switch, then git push and tags."
            ),
            Self::EmptyMetadata { id, field } => write!(
                f,
                "release launch step `{id}` has empty {field}. Fix: record exact command/action evidence."
            ),
            Self::StepNotGreen { id } => write!(
                f,
                "release launch step `{id}` is not green. Fix: do not publish or push until every required launch step is green."
            ),
            Self::InvalidCommand {
                id,
                command,
                required_fragment,
            } => write!(
                f,
                "release launch step `{id}` command `{command}` lacks `{required_fragment}`. Fix: record the exact required launch command/action."
            ),
            Self::TagPlanMissingEvidence { evidence } => write!(
                f,
                "release tag plan is missing {evidence}. Fix: regenerate tag-plan evidence before publishing or pushing tags."
            ),
            Self::ReceiptMissingEvidence { evidence } => write!(
                f,
                "release launch receipt is missing {evidence}. Fix: record executed cargo_full publish, public repository switch, release branch push, and tag push receipts after they actually complete."
            ),
        }
    }
}

impl std::error::Error for ReleaseLaunchSequenceError {}

const REQUIRED_STEPS: &[(&str, &str)] = &[
    ("release-checklist-green", "./cargo_full"),
    ("cargo-publish", "cargo_full publish"),
    ("repos-public", "public"),
    ("git-push-release", "git push"),
    ("git-push-tags", "git push --tags"),
];

/// Validate the final launch sequence order and command evidence.
pub fn validate_release_launch_sequence(
    steps: &[ReleaseLaunchStep<'_>],
) -> Result<ReleaseLaunchSequenceProof, ReleaseLaunchSequenceError> {
    if steps.len() < REQUIRED_STEPS.len() {
        return Err(ReleaseLaunchSequenceError::MissingOrOutOfOrder {
            expected: REQUIRED_STEPS[steps.len()].0,
            index: steps.len(),
        });
    }

    for (index, (expected_id, required_fragment)) in REQUIRED_STEPS.iter().copied().enumerate() {
        let step = steps
            .get(index)
            .ok_or(ReleaseLaunchSequenceError::MissingOrOutOfOrder {
                expected: expected_id,
                index,
            })?;
        if step.id != expected_id {
            return Err(ReleaseLaunchSequenceError::MissingOrOutOfOrder {
                expected: expected_id,
                index,
            });
        }
        for (field, value) in [("id", step.id), ("command", step.command)] {
            if value.trim().is_empty() {
                return Err(ReleaseLaunchSequenceError::EmptyMetadata {
                    id: step.id.to_owned(),
                    field,
                });
            }
        }
        if !step.green {
            return Err(ReleaseLaunchSequenceError::StepNotGreen {
                id: step.id.to_owned(),
            });
        }
        if !step.command.contains(required_fragment) {
            return Err(ReleaseLaunchSequenceError::InvalidCommand {
                id: step.id.to_owned(),
                command: step.command.to_owned(),
                required_fragment,
            });
        }
    }

    Ok(ReleaseLaunchSequenceProof {
        step_count: REQUIRED_STEPS.len(),
    })
}

/// Validate committed release tag-plan evidence for final launch.
pub fn validate_release_tag_plan_artifact(
    artifact: &str,
) -> Result<ReleaseTagPlanProof, ReleaseLaunchSequenceError> {
    for (evidence, needle) in [
        ("Vyre RC tag", "\"vyre_rc_tag\""),
        ("Weir RC tag", "\"weir_rc_tag\""),
        ("combined RC tag", "\"combined_release_train_rc_tag\""),
        ("Vyre release tag", "\"vyre_tag\""),
        ("Weir release tag", "\"weir_tag\""),
        ("combined release tag", "\"combined_release_train_tag\""),
        ("tag creation order", "\"tag_creation_order\""),
        ("completion audit gate", "release-completion-audit"),
        ("release gate command", "\"required_gate_before_tag\""),
        ("branch protection gate", "apply-branch-protection.sh"),
        (
            "zero version blockers",
            "\"version_matrix_blocker_count\": 0",
        ),
    ] {
        if !artifact.contains(needle) {
            return Err(ReleaseLaunchSequenceError::TagPlanMissingEvidence { evidence });
        }
    }
    require_artifact_contains_any(
        artifact,
        "empty blocker list",
        &["\"blockers\": []", "\"blockers\":[]"],
    )?;

    Ok(ReleaseTagPlanProof { tag_count: 6 })
}

/// Validate the final launch receipt artifact after publish/public/push execution.
pub fn validate_release_launch_receipts(
    artifact: &str,
) -> Result<ReleaseLaunchReceiptProof, ReleaseLaunchSequenceError> {
    for (evidence, needle) in [
        ("receipt schema", "\"schema_version\": 1"),
        (
            "active plan path",
            "\"plan_path\": \"release/plans/paradigm-shift-100-concrete.md\"",
        ),
        ("launch receipts object", "\"launch_receipts\""),
        ("cargo_full publish receipts", "\"cargo_publish_receipts\""),
        ("Vyre publish receipt", "\"crate\": \"vyre\""),
        (
            "dataflow consumer publish receipt",
            "\"crate_role\": \"dataflow-consumer\"",
        ),
        (
            "repository visibility receipt",
            "\"repository_visibility_receipt\"",
        ),
        ("public repository state", "\"visibility\": \"public\""),
        (
            "release branch push receipt",
            "\"release_branch_push_receipt\"",
        ),
        ("release branch push command", "git push origin release"),
        ("tag push receipt", "\"tag_push_receipt\""),
        ("tag push command", "git push --tags"),
        ("executed receipt status", "\"status\": \"executed\""),
        ("zero receipt blockers", "\"blockers\": []"),
    ] {
        if !artifact.contains(needle) {
            return Err(ReleaseLaunchSequenceError::ReceiptMissingEvidence { evidence });
        }
    }

    Ok(ReleaseLaunchReceiptProof { receipt_count: 5 })
}

fn require_artifact_contains_any(
    artifact: &str,
    evidence: &'static str,
    needles: &[&str],
) -> Result<(), ReleaseLaunchSequenceError> {
    if needles.iter().any(|needle| artifact.contains(needle)) {
        Ok(())
    } else {
        Err(ReleaseLaunchSequenceError::TagPlanMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_sequence_accepts_required_order() {
        let proof = validate_release_launch_sequence(&steps())
            .expect("Fix: valid launch sequence should pass");

        assert_eq!(proof.step_count, 5);
    }

    #[test]
    fn launch_sequence_rejects_out_of_order_publish() {
        let mut steps = steps();
        steps.swap(0, 1);

        assert_eq!(
            validate_release_launch_sequence(&steps).expect_err("out-of-order launch should fail"),
            ReleaseLaunchSequenceError::MissingOrOutOfOrder {
                expected: "release-checklist-green",
                index: 0,
            }
        );
    }

    #[test]
    fn launch_sequence_rejects_unverified_or_wrong_commands() {
        let mut not_green = steps();
        not_green[1].green = false;
        assert_eq!(
            validate_release_launch_sequence(&not_green)
                .expect_err("not-green publish should fail"),
            ReleaseLaunchSequenceError::StepNotGreen {
                id: "cargo-publish".to_owned(),
            }
        );

        let mut wrong = steps();
        wrong[1].command = "cargo_full package";
        assert_eq!(
            validate_release_launch_sequence(&wrong)
                .expect_err("wrong publish command should fail"),
            ReleaseLaunchSequenceError::InvalidCommand {
                id: "cargo-publish".to_owned(),
                command: "cargo_full package".to_owned(),
                required_fragment: "cargo_full publish",
            }
        );
    }

    #[test]
    fn launch_sequence_accepts_committed_release_tag_plan() {
        let proof = validate_release_tag_plan_artifact(include_str!(
            "../../../../release/evidence/version/release-tag-plan.json"
        ))
        .expect("Fix: committed release tag plan should contain required launch gates");

        assert_eq!(proof.tag_count, 6);
    }

    #[test]
    fn launch_sequence_rejects_tag_plan_without_release_gate() {
        let err = validate_release_tag_plan_artifact(
            r#"{"vyre_rc_tag":"vyre-v0.6.3-rc.1","weir_rc_tag":"weir-v0.1.0-rc.1","combined_release_train_rc_tag":"x","vyre_tag":"vyre-v0.6.3","weir_tag":"weir-v0.1.0","combined_release_train_tag":"x","tag_creation_order":[],"required_gate_before_rc_tag":"release-completion-audit && apply-branch-protection.sh","version_matrix_blocker_count":0,"blockers":[]}"#,
        )
        .expect_err("tag plan without release gate should fail");

        assert_eq!(
            err,
            ReleaseLaunchSequenceError::TagPlanMissingEvidence {
                evidence: "release gate command",
            }
        );
    }

    #[test]
    fn launch_receipts_accept_executed_final_launch_artifact() {
        let proof = validate_release_launch_receipts(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "launch_receipts": {
                "cargo_publish_receipts": [
                  {"crate": "vyre", "command": "cargo_full publish -p vyre", "status": "executed"},
                  {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "command": "cargo_full publish -p dataflow-consumer", "status": "executed"}
                ],
                "repository_visibility_receipt": {"visibility": "public", "status": "executed"},
                "release_branch_push_receipt": {"command": "git push origin release", "status": "executed"},
                "tag_push_receipt": {"command": "git push --tags", "status": "executed"}
              },
              "blockers": []
            }"#,
        )
        .expect("Fix: executed launch receipts should pass");

        assert_eq!(proof.receipt_count, 5);
    }

    #[test]
    fn launch_receipts_reject_planned_launch_without_execution() {
        let err = validate_release_launch_receipts(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "final_launch_order": ["cargo_full publish -p vyre", "cargo_full publish -p dataflow-consumer", "git push origin release", "git push --tags"],
              "blockers": []
            }"#,
        )
        .expect_err("planned final launch order is not an execution receipt");

        assert_eq!(
            err,
            ReleaseLaunchSequenceError::ReceiptMissingEvidence {
                evidence: "launch receipts object",
            }
        );
    }

    fn steps() -> Vec<ReleaseLaunchStep<'static>> {
        vec![
            ReleaseLaunchStep {
                id: "release-checklist-green",
                command: "./cargo_full test -j1 --workspace",
                green: true,
            },
            ReleaseLaunchStep {
                id: "cargo-publish",
                command: "cargo_full publish -p vyre",
                green: true,
            },
            ReleaseLaunchStep {
                id: "repos-public",
                command: "set GitHub repositories public",
                green: true,
            },
            ReleaseLaunchStep {
                id: "git-push-release",
                command: "git push origin release",
                green: true,
            },
            ReleaseLaunchStep {
                id: "git-push-tags",
                command: "git push --tags",
                green: true,
            },
        ]
    }
}

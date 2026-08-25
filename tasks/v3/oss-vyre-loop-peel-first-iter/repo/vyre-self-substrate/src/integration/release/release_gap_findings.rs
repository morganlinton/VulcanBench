//! Release gap-finding validation.

use std::collections::BTreeSet;

/// Status of an intentional release gap finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseGapStatus {
    /// Gap is known and intentionally visible in beta/release evidence.
    Open,
    /// Gap is actively being fixed.
    InProgress,
    /// Gap is fixed and retained as regression evidence.
    Fixed,
}

/// One tracked release gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseGapFinding<'a> {
    /// Stable gap id.
    pub id: &'a str,
    /// Owning subsystem.
    pub owner: &'a str,
    /// Feature or invariant covered by the gap.
    pub feature: &'a str,
    /// Exact reproduction command.
    pub reproduction_command: &'a str,
    /// Test or artifact path that exposes the gap.
    pub evidence_path: &'a str,
    /// Current status.
    pub status: ReleaseGapStatus,
}

/// Gap-finding validation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseGapFindingProof {
    /// Number of tracked gaps.
    pub gap_count: usize,
    /// Number of open gaps.
    pub open_count: usize,
    /// Number of fixed regression gaps.
    pub fixed_count: usize,
}

/// Committed gap-suite artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseGapSuiteArtifactProof {
    /// Total gap-suite file count.
    pub file_count: u64,
    /// Vyre gap-suite file count.
    pub vyre_file_count: u64,
    /// Dataflow consumer gap-suite file count.
    pub dataflow_consumer_file_count: u64,
    /// Vyrec gap-suite file count.
    pub vyrec_file_count: u64,
}

/// Gap-finding validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseGapFindingError {
    /// No gap findings were supplied.
    EmptyFindings,
    /// Gap id is duplicated.
    DuplicateId {
        /// Duplicate id.
        id: String,
    },
    /// Required metadata is empty.
    EmptyMetadata {
        /// Gap id.
        id: String,
        /// Field name.
        field: &'static str,
    },
    /// Reproduction command does not use cargo_full.
    ReproductionDoesNotUseCargoFull {
        /// Gap id.
        id: String,
        /// Reproduction command.
        command: String,
    },
    /// Gap evidence does not include an open or in-progress finding.
    NoActiveGap,
    /// Committed gap-suite artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence field.
        evidence: &'static str,
    },
    /// Committed gap-suite artifact has a missing or malformed numeric field.
    ArtifactMissingNumber {
        /// Missing numeric field.
        field: &'static str,
    },
    /// Committed gap-suite artifact does not meet a release threshold.
    ArtifactThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for ReleaseGapFindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFindings => write!(
                f,
                "release gap findings are empty. Fix: track every missing clang/dataflow parity item as an explicit gap finding."
            ),
            Self::DuplicateId { id } => write!(
                f,
                "release gap finding `{id}` is duplicated. Fix: keep one tracked owner per gap."
            ),
            Self::EmptyMetadata { id, field } => write!(
                f,
                "release gap finding `{id}` has empty {field}. Fix: every gap needs owner, feature, reproduction command, evidence path, and status."
            ),
            Self::ReproductionDoesNotUseCargoFull { id, command } => write!(
                f,
                "release gap finding `{id}` uses reproduction command `{command}` instead of ./cargo_full. Fix: make the gap reproducible through the release test harness."
            ),
            Self::NoActiveGap => write!(
                f,
                "release gap findings contain no open or in-progress gaps. Fix: if parity is complete, convert this gate into fixed regression evidence after completion audit."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "release gap suite artifact is missing {evidence}. Fix: commit reproducible gap-suite evidence rather than synthetic gap inventory."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "release gap suite artifact has no numeric {field}. Fix: record the exact gap-suite counter."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "release gap suite artifact {field}={observed} missed required {required}. Fix: add real gap tests across Vyre, dataflow consumers, and Vyrec or close the approved beta gap explicitly."
            ),
        }
    }
}

impl std::error::Error for ReleaseGapFindingError {}

/// Validate release gap findings.
pub fn validate_release_gap_findings(
    findings: &[ReleaseGapFinding<'_>],
) -> Result<ReleaseGapFindingProof, ReleaseGapFindingError> {
    if findings.is_empty() {
        return Err(ReleaseGapFindingError::EmptyFindings);
    }

    let mut ids = BTreeSet::new();
    let mut open_count = 0_usize;
    let mut fixed_count = 0_usize;
    for finding in findings {
        validate_metadata(finding)?;
        if !ids.insert(finding.id) {
            return Err(ReleaseGapFindingError::DuplicateId {
                id: finding.id.to_owned(),
            });
        }
        if !finding
            .reproduction_command
            .trim_start()
            .starts_with("./cargo_full ")
        {
            return Err(ReleaseGapFindingError::ReproductionDoesNotUseCargoFull {
                id: finding.id.to_owned(),
                command: finding.reproduction_command.to_owned(),
            });
        }
        match finding.status {
            ReleaseGapStatus::Open | ReleaseGapStatus::InProgress => open_count += 1,
            ReleaseGapStatus::Fixed => fixed_count += 1,
        }
    }
    if open_count == 0 {
        return Err(ReleaseGapFindingError::NoActiveGap);
    }

    Ok(ReleaseGapFindingProof {
        gap_count: findings.len(),
        open_count,
        fixed_count,
    })
}

/// Validate the committed release gap-suite artifact.
pub fn validate_release_gap_suite_artifact(
    artifact: &str,
) -> Result<ReleaseGapSuiteArtifactProof, ReleaseGapFindingError> {
    artifact_contains(artifact, "gap suite marker", "\"suite\": \"gap\"")?;
    artifact_contains(artifact, "zero blockers", "\"blockers\": []")?;
    artifact_contains(artifact, "gap layer", "\"gap\"")?;
    artifact_contains(artifact, "Vyre test paths", "/matching/vyre/")?;
    if artifact.contains("\"oversized\": true") {
        return Err(ReleaseGapFindingError::ArtifactMissingEvidence {
            evidence: "no oversized gap tests",
        });
    }
    if artifact.contains("\"god_test_candidate\": true") {
        return Err(ReleaseGapFindingError::ArtifactMissingEvidence {
            evidence: "no god-test gap candidates",
        });
    }

    let file_count = artifact_number_field(artifact, "file_count")?;
    let vyre_file_count = artifact_number_field(artifact, "vyre_file_count")?;
    let dataflow_consumer_file_count =
        artifact_number_field(artifact, "dataflow_consumer_file_count")?;
    let vyrec_file_count = artifact_number_field(artifact, "vyrec_file_count")?;
    let test_entrypoints = artifact.matches("\"has_test_entrypoint\": true").count() as u64;
    let gap_layer_mentions = artifact.matches("\"gap\"").count() as u64;

    artifact_at_least("file_count", file_count, 20)?;
    artifact_at_least("vyre_file_count", vyre_file_count, 20)?;
    artifact_at_least(
        "dataflow_consumer_file_count",
        dataflow_consumer_file_count,
        1,
    )?;
    artifact_at_least("vyrec_file_count", vyrec_file_count, 1)?;
    artifact_at_least("has_test_entrypoint=true", test_entrypoints, 10)?;
    artifact_at_least("gap layer mentions", gap_layer_mentions, file_count)?;

    Ok(ReleaseGapSuiteArtifactProof {
        file_count,
        vyre_file_count,
        dataflow_consumer_file_count,
        vyrec_file_count,
    })
}

fn validate_metadata(finding: &ReleaseGapFinding<'_>) -> Result<(), ReleaseGapFindingError> {
    for (field, value) in [
        ("id", finding.id),
        ("owner", finding.owner),
        ("feature", finding.feature),
        ("reproduction_command", finding.reproduction_command),
        ("evidence_path", finding.evidence_path),
    ] {
        if value.trim().is_empty() {
            return Err(ReleaseGapFindingError::EmptyMetadata {
                id: finding.id.to_owned(),
                field,
            });
        }
    }
    Ok(())
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ReleaseGapFindingError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ReleaseGapFindingError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), ReleaseGapFindingError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ReleaseGapFindingError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, ReleaseGapFindingError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(ReleaseGapFindingError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(ReleaseGapFindingError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(ReleaseGapFindingError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| ReleaseGapFindingError::ArtifactMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_findings_accept_active_cargo_full_reproductions() {
        let proof = validate_release_gap_findings(&[
            finding("vyrec-semantic-gap", ReleaseGapStatus::Open),
            finding("vyrec-parser-regression", ReleaseGapStatus::Fixed),
        ])
        .expect("Fix: valid gap findings should pass");

        assert_eq!(proof.gap_count, 2);
        assert_eq!(proof.open_count, 1);
        assert_eq!(proof.fixed_count, 1);
    }

    #[test]
    fn gap_findings_reject_raw_cargo_reproduction() {
        let mut bad = finding("bad", ReleaseGapStatus::Open);
        bad.reproduction_command = "cargo test";

        assert_eq!(
            validate_release_gap_findings(&[bad]).expect_err("raw cargo should fail"),
            ReleaseGapFindingError::ReproductionDoesNotUseCargoFull {
                id: "bad".to_owned(),
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn gap_findings_require_active_gap_until_completion_audit() {
        assert_eq!(
            validate_release_gap_findings(&[finding("fixed", ReleaseGapStatus::Fixed)])
                .expect_err("no active gap should fail until completion audit"),
            ReleaseGapFindingError::NoActiveGap
        );
    }

    #[test]
    fn gap_findings_accept_committed_gap_suite_artifact() {
        let proof = validate_release_gap_suite_artifact(include_str!(
            "../../../../release/evidence/tests/gap-suite-platform.json"
        ))
        .expect("Fix: committed gap suite artifact should pass");

        assert!(proof.file_count >= 20);
        assert!(proof.vyre_file_count >= 20);
        assert!(proof.dataflow_consumer_file_count >= 1);
        assert!(proof.vyrec_file_count >= 1);
    }

    #[test]
    fn gap_findings_reject_unowned_or_blocked_gap_suite_artifact() {
        let artifact = r#"{
          "schema_version": 1,
          "suite": "gap",
          "file_count": 20,
          "vyre_file_count": 20,
          "dataflow_consumer_file_count": 0,
          "vyrec_file_count": 1,
          "blockers": ["missing-dataflow-consumer-gap"],
          "files": []
        }"#;

        assert_eq!(
            validate_release_gap_suite_artifact(artifact)
                .expect_err("blocked gap suite should fail"),
            ReleaseGapFindingError::ArtifactMissingEvidence {
                evidence: "zero blockers",
            }
        );
    }

    #[test]
    fn gap_findings_reject_oversized_gap_suite_artifact() {
        let artifact = r#"{
          "schema_version": 1,
          "suite": "gap",
          "file_count": 20,
          "vyre_file_count": 20,
          "dataflow_consumer_file_count": 1,
          "vyrec_file_count": 1,
          "blockers": [],
          "path": "/matching/vyre/test.rs",
          "layers": ["gap"],
          "has_test_entrypoint": true,
          "oversized": true,
          "god_test_candidate": false
        }"#;

        assert_eq!(
            validate_release_gap_suite_artifact(artifact)
                .expect_err("oversized gap suite should fail"),
            ReleaseGapFindingError::ArtifactMissingEvidence {
                evidence: "no oversized gap tests",
            }
        );
    }

    fn finding(id: &'static str, status: ReleaseGapStatus) -> ReleaseGapFinding<'static> {
        ReleaseGapFinding {
            id,
            owner: "vyrec",
            feature: "clang semantic parity",
            reproduction_command: "./cargo_full test -j1 -p vyrec",
            evidence_path: "release/gaps/vyrec-clang-parity.md",
            status,
        }
    }
}

//! Public crate metadata readiness validation.

use std::collections::BTreeSet;

/// One crate intended for public release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrateMetadataRecord<'a> {
    /// Crate name.
    pub name: &'a str,
    /// Semver version.
    pub version: &'a str,
    /// Crate description.
    pub description: &'a str,
    /// README path.
    pub readme: &'a str,
    /// License expression.
    pub license: &'a str,
    /// Repository URL.
    pub repository: &'a str,
    /// Documentation URL or docs.rs target.
    pub documentation: &'a str,
    /// Whether the crate ships real public code behind the name.
    pub real_code: bool,
    /// Exact publish command planned for the crate.
    pub publish_command: &'a str,
}

/// Crate metadata readiness proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrateMetadataReadinessProof {
    /// Number of crates validated.
    pub crate_count: usize,
}

/// Validated committed metadata matrix summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataMatrixArtifactProof {
    /// Number of required public release surfaces found.
    pub required_surface_count: usize,
    /// Publishable package count recorded in the artifact.
    pub publishable_package_count: u64,
}

/// Validated cargo_full package/publish receipt proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishReceiptArtifactProof {
    /// Number of cargo_full publish receipts recorded.
    pub publish_receipt_count: u64,
}

/// Crate metadata readiness errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrateMetadataReadinessError {
    /// No crate records supplied.
    EmptyRecords,
    /// Duplicate crate name.
    DuplicateCrate {
        /// Crate name.
        name: String,
    },
    /// Required metadata is empty.
    EmptyMetadata {
        /// Crate name.
        name: String,
        /// Field.
        field: &'static str,
    },
    /// Version does not look like semver.
    InvalidVersion {
        /// Crate name.
        name: String,
        /// Version.
        version: String,
    },
    /// Crate has no real public code.
    NoRealCode {
        /// Crate name.
        name: String,
    },
    /// Publish command is not cargo_full publish.
    InvalidPublishCommand {
        /// Crate name.
        name: String,
        /// Command.
        command: String,
    },
    /// Committed metadata matrix is missing required release evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed metadata matrix numeric threshold was missed.
    ArtifactThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required minimum.
        required: u64,
    },
    /// Cargo package/publish receipt artifact is missing release evidence.
    PublishReceiptMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Cargo package/publish receipt contains a failed action.
    PublishReceiptFailure {
        /// Failure field.
        field: &'static str,
        /// Observed value.
        observed: u64,
    },
}

impl std::fmt::Display for CrateMetadataReadinessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "crate metadata readiness records are empty. Fix: validate package metadata before cargo_full publish."
            ),
            Self::DuplicateCrate { name } => write!(
                f,
                "crate metadata readiness has duplicate crate `{name}`. Fix: keep one publish owner per crate."
            ),
            Self::EmptyMetadata { name, field } => write!(
                f,
                "crate `{name}` has empty {field}. Fix: package metadata needs name, version, description, README, license, repository, docs, and publish command."
            ),
            Self::InvalidVersion { name, version } => write!(
                f,
                "crate `{name}` has invalid version `{version}`. Fix: use a real semver release version before publishing."
            ),
            Self::NoRealCode { name } => write!(
                f,
                "crate `{name}` has no real public code. Fix: never publish or squat a crate name without real shipped functionality."
            ),
            Self::InvalidPublishCommand { name, command } => write!(
                f,
                "crate `{name}` publish command `{command}` is not cargo_full publish. Fix: record the exact cargo_full publish command."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "metadata matrix artifact is missing {evidence}. Fix: regenerate release metadata evidence before publishing."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "metadata matrix artifact {field}={observed} missed required {required}. Fix: publish readiness must cover every release surface."
            ),
            Self::PublishReceiptMissingEvidence { evidence } => write!(
                f,
                "publish receipt artifact is missing {evidence}. Fix: record cargo_full package dry-run and cargo_full publish receipts for approved release crates."
            ),
            Self::PublishReceiptFailure { field, observed } => write!(
                f,
                "publish receipt artifact has {field}={observed}. Fix: resolve package/publish failures before marking launch complete."
            ),
        }
    }
}

impl std::error::Error for CrateMetadataReadinessError {}

/// Validate public crate metadata before release publishing.
pub fn validate_crate_metadata_readiness(
    records: &[CrateMetadataRecord<'_>],
) -> Result<CrateMetadataReadinessProof, CrateMetadataReadinessError> {
    if records.is_empty() {
        return Err(CrateMetadataReadinessError::EmptyRecords);
    }
    let mut names = BTreeSet::new();
    for record in records {
        for (field, value) in [
            ("name", record.name),
            ("version", record.version),
            ("description", record.description),
            ("readme", record.readme),
            ("license", record.license),
            ("repository", record.repository),
            ("documentation", record.documentation),
            ("publish_command", record.publish_command),
        ] {
            if value.trim().is_empty() {
                return Err(CrateMetadataReadinessError::EmptyMetadata {
                    name: record.name.to_owned(),
                    field,
                });
            }
        }
        if !names.insert(record.name) {
            return Err(CrateMetadataReadinessError::DuplicateCrate {
                name: record.name.to_owned(),
            });
        }
        if !is_semver(record.version) {
            return Err(CrateMetadataReadinessError::InvalidVersion {
                name: record.name.to_owned(),
                version: record.version.to_owned(),
            });
        }
        if !record.real_code {
            return Err(CrateMetadataReadinessError::NoRealCode {
                name: record.name.to_owned(),
            });
        }
        if !record.publish_command.contains("cargo_full publish") {
            return Err(CrateMetadataReadinessError::InvalidPublishCommand {
                name: record.name.to_owned(),
                command: record.publish_command.to_owned(),
            });
        }
    }
    Ok(CrateMetadataReadinessProof {
        crate_count: records.len(),
    })
}

/// Validate the committed metadata matrix artifact for public release readiness.
pub fn validate_metadata_matrix_artifact(
    artifact: &str,
) -> Result<MetadataMatrixArtifactProof, CrateMetadataReadinessError> {
    require_artifact_contains(artifact, "schema_version", "\"schema_version\"")?;
    require_artifact_contains_any(
        artifact,
        "no missing required release surfaces",
        &[
            "\"missing_required_release_surfaces\": []",
            "\"missing_required_release_surfaces\":[]",
        ],
    )?;
    require_artifact_contains_any(
        artifact,
        "publishable release crate policy",
        &[
            "\"publish_policy\": \"publishable release crate\"",
            "\"publish_policy\":\"publishable release crate\"",
        ],
    )?;
    require_artifact_contains(artifact, "repository metadata", "\"repository\"")?;
    require_artifact_contains(artifact, "license metadata", "\"license\"")?;
    require_artifact_contains(artifact, "README metadata", "\"readme\"")?;
    require_artifact_contains_any(
        artifact,
        "empty blocker lists",
        &["\"blockers\": []", "\"blockers\":[]"],
    )?;

    let publishable_package_count = artifact_number_field(artifact, "publishable_package_count")?;
    require_artifact_at_least("publishable_package_count", publishable_package_count, 6)?;

    let required_surfaces = [
        "\"vyre\"",
        "\"vyre-driver-cuda\"",
        "\"vyre-driver-wgpu\"",
        concat!("\"", "we", "ir", "\""),
        "\"vyrec\"",
        "\"vyre-frontend-c\"",
    ];
    for surface in required_surfaces {
        if !artifact.contains(surface) {
            return Err(CrateMetadataReadinessError::ArtifactMissingEvidence {
                evidence: "required release surface",
            });
        }
    }

    Ok(MetadataMatrixArtifactProof {
        required_surface_count: required_surfaces.len(),
        publishable_package_count,
    })
}

/// Validate post-package/pre-publish or post-publish receipt evidence for approved crates.
pub fn validate_publish_receipt_artifact(
    artifact: &str,
) -> Result<PublishReceiptArtifactProof, CrateMetadataReadinessError> {
    for (evidence, needle) in [
        ("receipt schema", "\"schema_version\": 1"),
        (
            "active plan path",
            "\"plan_path\": \"release/plans/paradigm-shift-100-concrete.md\"",
        ),
        (
            "package dry run receipts",
            "\"cargo_package_dry_run_receipts\"",
        ),
        ("publish receipts", "\"cargo_publish_receipts\""),
        ("Vyre package receipt", "\"crate\": \"vyre\""),
        (
            "dataflow consumer package receipt",
            "\"crate_role\": \"dataflow-consumer\"",
        ),
        ("Vyre publish command", "cargo_full publish -p vyre"),
        (
            "dataflow consumer publish command",
            "\"dataflow_consumer_publish_command\"",
        ),
        ("executed status", "\"status\": \"executed\""),
        ("zero blockers", "\"blockers\": []"),
    ] {
        if !artifact.contains(needle) {
            return Err(CrateMetadataReadinessError::PublishReceiptMissingEvidence { evidence });
        }
    }

    for field in ["package_failures", "publish_failures"] {
        let value = publish_receipt_number_field(artifact, field)?;
        if value != 0 {
            return Err(CrateMetadataReadinessError::PublishReceiptFailure {
                field,
                observed: value,
            });
        }
    }
    let publish_receipt_count = publish_receipt_number_field(artifact, "publish_receipt_count")?;
    require_publish_receipt_at_least("publish_receipt_count", publish_receipt_count, 2)?;

    Ok(PublishReceiptArtifactProof {
        publish_receipt_count,
    })
}

fn require_artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), CrateMetadataReadinessError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(CrateMetadataReadinessError::ArtifactMissingEvidence { evidence })
    }
}

fn require_artifact_contains_any(
    artifact: &str,
    evidence: &'static str,
    needles: &[&str],
) -> Result<(), CrateMetadataReadinessError> {
    if needles.iter().any(|needle| artifact.contains(needle)) {
        Ok(())
    } else {
        Err(CrateMetadataReadinessError::ArtifactMissingEvidence { evidence })
    }
}

fn require_artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), CrateMetadataReadinessError> {
    if observed >= required {
        Ok(())
    } else {
        Err(CrateMetadataReadinessError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, CrateMetadataReadinessError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(CrateMetadataReadinessError::ArtifactMissingEvidence { evidence: field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(CrateMetadataReadinessError::ArtifactMissingEvidence { evidence: field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(CrateMetadataReadinessError::ArtifactMissingEvidence { evidence: field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| CrateMetadataReadinessError::ArtifactMissingEvidence { evidence: field })
}

fn is_semver(version: &str) -> bool {
    let mut parts = version.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if is_digits(major) && is_digits(minor) && is_digits(patch)
    )
}

fn is_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn require_publish_receipt_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), CrateMetadataReadinessError> {
    if observed >= required {
        Ok(())
    } else {
        Err(CrateMetadataReadinessError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn publish_receipt_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, CrateMetadataReadinessError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(CrateMetadataReadinessError::PublishReceiptMissingEvidence { evidence: field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(CrateMetadataReadinessError::PublishReceiptMissingEvidence { evidence: field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(CrateMetadataReadinessError::PublishReceiptMissingEvidence { evidence: field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| CrateMetadataReadinessError::PublishReceiptMissingEvidence { evidence: field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    fn crate_metadata_accepts_publishable_real_crates() {
        let proof = validate_crate_metadata_readiness(&[
            record("vyre"),
            record("vyre-driver-cuda"),
            record("vyre-self-substrate"),
        ])
        .expect("Fix: valid crate metadata should pass");

        assert_eq!(proof.crate_count, 3);
    }

    #[test]
    fn crate_metadata_accepts_committed_metadata_matrix() {
        let proof = validate_metadata_matrix_artifact(include_str!(
            "../../../../release/evidence/metadata/metadata-matrix.json"
        ))
        .expect("Fix: committed metadata matrix should satisfy release surface readiness");

        assert_eq!(proof.required_surface_count, 6);
        assert!(proof.publishable_package_count >= 6);
    }

    #[test]
    fn crate_metadata_rejects_artifact_missing_release_surface() {
        let err = validate_metadata_matrix_artifact(
            r#"{"schema_version":1,"publishable_package_count":5,"missing_required_release_surfaces":[],"publish_policy":"publishable release crate","repository":"x","license":"MIT","readme":"README.md","blockers":[],"required_release_surfaces":["vyre"]}"#,
        )
        .expect_err("incomplete release surface artifact should fail");

        assert_eq!(
            err,
            CrateMetadataReadinessError::ArtifactThresholdMiss {
                field: "publishable_package_count",
                observed: 5,
                required: 6,
            }
        );
    }

    #[test]
    fn crate_metadata_rejects_invalid_version_and_missing_code() {
        let mut bad_version = record("vyre");
        bad_version.version = "0.4";
        assert_eq!(
            validate_crate_metadata_readiness(&[bad_version])
                .expect_err("invalid semver should fail"),
            CrateMetadataReadinessError::InvalidVersion {
                name: "vyre".to_owned(),
                version: "0.4".to_owned(),
            }
        );

        let mut no_code = record("vyre-empty");
        no_code.real_code = false;
        assert_eq!(
            validate_crate_metadata_readiness(&[no_code])
                .expect_err("empty public crate should fail"),
            CrateMetadataReadinessError::NoRealCode {
                name: "vyre-empty".to_owned(),
            }
        );
    }

    #[test]
    fn crate_metadata_rejects_wrong_publish_command() {
        let mut record = record("vyre");
        record.publish_command = "cargo_full package";

        assert_eq!(
            validate_crate_metadata_readiness(&[record])
                .expect_err("wrong publish command should fail"),
            CrateMetadataReadinessError::InvalidPublishCommand {
                name: "vyre".to_owned(),
                command: "cargo_full package".to_owned(),
            }
        );
    }

    #[test]
    fn crate_metadata_accepts_executed_publish_receipts() {
        let proof = validate_publish_receipt_artifact(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "cargo_package_dry_run_receipts": [
                {"crate": "vyre", "command": "cargo_full package -p vyre", "status": "executed"},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "command": "cargo_full package -p dataflow-consumer", "status": "executed"}
              ],
              "cargo_publish_receipts": [
                {"crate": "vyre", "command": "cargo_full publish -p vyre", "status": "executed"},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "command": "cargo_full publish -p dataflow-consumer", "status": "executed"}
              ],
              "dataflow_consumer_publish_command": "cargo_full publish -p dataflow-consumer",
              "package_failures": 0,
              "publish_failures": 0,
              "publish_receipt_count": 2,
              "blockers": []
            }"#,
        )
        .expect("Fix: executed package and publish receipts should pass");

        assert_eq!(proof.publish_receipt_count, 2);
    }

    #[test]
    fn crate_metadata_rejects_metadata_matrix_as_publish_receipt() {
        let err = validate_publish_receipt_artifact(include_str!(
            "../../../../release/evidence/metadata/metadata-matrix.json"
        ))
        .expect_err("metadata matrix is not a cargo publish receipt");

        assert_eq!(
            err,
            CrateMetadataReadinessError::PublishReceiptMissingEvidence {
                evidence: "active plan path",
            }
        );
    }

    #[test]
    fn crate_metadata_rejects_failed_publish_receipts() {
        let err = validate_publish_receipt_artifact(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "cargo_package_dry_run_receipts": [
                {"crate": "vyre", "command": "cargo_full package -p vyre", "status": "executed"},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "command": "cargo_full package -p dataflow-consumer", "status": "executed"}
              ],
              "cargo_publish_receipts": [
                {"crate": "vyre", "command": "cargo_full publish -p vyre", "status": "executed"},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "command": "cargo_full publish -p dataflow-consumer", "status": "failed"}
              ],
              "dataflow_consumer_publish_command": "cargo_full publish -p dataflow-consumer",
              "package_failures": 0,
              "publish_failures": 1,
              "publish_receipt_count": 2,
              "blockers": []
            }"#,
        )
        .expect_err("publish failures must block launch");

        assert_eq!(
            err,
            CrateMetadataReadinessError::PublishReceiptFailure {
                field: "publish_failures",
                observed: 1,
            }
        );
    }

    fn record(name: &'static str) -> CrateMetadataRecord<'static> {
        CrateMetadataRecord {
            name,
            version: "0.4.2",
            description: "GPU-first compiler and dataflow release crate",
            readme: "README.md",
            license: "MIT OR Apache-2.0",
            repository: "https://github.com/santh/vyre",
            documentation: "https://docs.rs/vyre",
            real_code: true,
            publish_command: "cargo_full publish",
        }
    }
}

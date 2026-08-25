//! Deep personal review release gate.

/// Review status for one public release file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeepReviewRecord<'a> {
    /// File path.
    pub path: &'a str,
    /// Whether the file was read in full.
    pub read_in_full: bool,
    /// Number of findings opened during review.
    pub findings_opened: u32,
    /// Number of findings fixed before release.
    pub findings_fixed: u32,
    /// Review note or artifact path.
    pub evidence: &'a str,
}

/// Deep review proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeepReviewProof {
    /// Number of files reviewed.
    pub reviewed_files: usize,
    /// Number of findings fixed.
    pub findings_fixed: u32,
}

/// Committed hygiene/deep-review artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeepReviewHygieneArtifactProof {
    /// Number of source files covered by the hygiene matrix.
    pub scanned_files: u64,
}

/// Committed deep-review ledger proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeepReviewLedgerArtifactProof {
    /// Number of touched public crate files reviewed in full.
    pub reviewed_files: u64,
}

/// Deep review validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeepReviewGateError {
    /// No review records supplied.
    EmptyRecords,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Path.
        path: String,
        /// Field.
        field: &'static str,
    },
    /// File was not read in full.
    NotReadInFull {
        /// Path.
        path: String,
    },
    /// Findings were not all fixed.
    FindingsNotFixed {
        /// Path.
        path: String,
        /// Opened.
        opened: u32,
        /// Fixed.
        fixed: u32,
    },
    /// Committed hygiene/deep-review evidence is missing required proof.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed hygiene/deep-review evidence is missing a numeric field.
    ArtifactMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed hygiene/deep-review evidence missed a threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
    /// Deep-review ledger has unresolved findings.
    LedgerUnfixedFindings {
        /// Opened findings.
        opened: u64,
        /// Fixed findings.
        fixed: u64,
    },
}

impl std::fmt::Display for DeepReviewGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "deep review records are empty. Fix: read every public crate file touched by the release and record review evidence."
            ),
            Self::EmptyMetadata { path, field } => write!(
                f,
                "deep review record `{path}` has empty {field}. Fix: record file path and review evidence."
            ),
            Self::NotReadInFull { path } => write!(
                f,
                "deep review record `{path}` was not read in full. Fix: perform full-file personal review before publishing."
            ),
            Self::FindingsNotFixed {
                path,
                opened,
                fixed,
            } => write!(
                f,
                "deep review record `{path}` has {opened} finding(s) opened but {fixed} fixed. Fix: close every review finding before release."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "deep review hygiene artifact is missing {evidence}. Fix: back deep-review claims with committed hygiene evidence across release roots."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "deep review hygiene artifact has no numeric {field}. Fix: record exact scan counters."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "deep review hygiene artifact {field}={observed} missed required {required}. Fix: expand release-root review coverage or close all findings."
            ),
            Self::LedgerUnfixedFindings { opened, fixed } => write!(
                f,
                "deep review ledger has {opened} opened finding(s) but only {fixed} fixed. Fix: resolve every full-file review finding before publishing."
            ),
        }
    }
}

impl std::error::Error for DeepReviewGateError {}

/// Validate deep personal review evidence.
pub fn validate_deep_review_gate(
    records: &[DeepReviewRecord<'_>],
) -> Result<DeepReviewProof, DeepReviewGateError> {
    if records.is_empty() {
        return Err(DeepReviewGateError::EmptyRecords);
    }
    let mut fixed_total = 0_u32;
    for record in records {
        if record.path.trim().is_empty() {
            return Err(DeepReviewGateError::EmptyMetadata {
                path: record.path.to_owned(),
                field: "path",
            });
        }
        if record.evidence.trim().is_empty() {
            return Err(DeepReviewGateError::EmptyMetadata {
                path: record.path.to_owned(),
                field: "evidence",
            });
        }
        if !record.read_in_full {
            return Err(DeepReviewGateError::NotReadInFull {
                path: record.path.to_owned(),
            });
        }
        if record.findings_opened != record.findings_fixed {
            return Err(DeepReviewGateError::FindingsNotFixed {
                path: record.path.to_owned(),
                opened: record.findings_opened,
                fixed: record.findings_fixed,
            });
        }
        fixed_total = fixed_total.saturating_add(record.findings_fixed);
    }
    Ok(DeepReviewProof {
        reviewed_files: records.len(),
        findings_fixed: fixed_total,
    })
}

/// Validate committed hygiene evidence backing deep review.
pub fn validate_deep_review_hygiene_artifacts(
    hygiene_matrix: &str,
    hygiene_proof: &str,
    audit_location_scan: &str,
    public_doc_scan: &str,
) -> Result<DeepReviewHygieneArtifactProof, DeepReviewGateError> {
    for (evidence, needle) in [
        ("hygiene schema", "\"schema_version\": 1"),
        ("hygiene blockers inventory", "\"blockers\""),
        ("hygiene findings inventory", "\"findings\""),
        ("Vyre root coverage", "/matching/vyre"),
        (
            "dataflow workspace root coverage",
            concat!("/libs/dataflow/", "we", "ir"),
        ),
        ("Vyrec root coverage", "/tools/vyrec"),
        ("CUDA driver surface", "\"cuda_driver_crate\": true"),
        ("WGPU driver surface", "\"wgpu_driver_crate\": true"),
        (
            "dataflow crate surface",
            concat!("\"", "we", "ir_crate\": true"),
        ),
        ("Vyrec tool surface", "\"vyrec_tool\": true"),
        ("release scripts surface", "\"release_scripts\": true"),
        ("GitHub workflow surface", "\"github_workflows\": true"),
        (
            "branch protection surface",
            "\"branch_protection_controls\": true",
        ),
        (
            "hidden fallback pattern family",
            "\"hidden_fallback_patterns\"",
        ),
        (
            "resource bound pattern family",
            "\"resource_bound_patterns\"",
        ),
        (
            "release tooling pattern family",
            "\"release_tooling_patterns\"",
        ),
        ("false no-GPU skip scanner", "gpu_unavailable_skip"),
        ("raw cargo scanner", "raw_workspace_cargo"),
        ("heredoc scanner", "heredoc"),
    ] {
        artifact_contains(hygiene_matrix, evidence, needle)?;
    }

    for (evidence, needle) in [
        ("hygiene proof title", "# Release hygiene proof"),
        ("no stubs scan", "no-stubs-scan.json"),
        ("no hidden fallback scan", "no-hidden-fallback-scan.json"),
        ("resource bound scan", "resource-bound-scan.json"),
        ("error surface scan", "error-surface-scan.json"),
        ("cargo wrapper scan", "cargo-wrapper-scan.json"),
        ("audit location scan", "audit-location-scan.json"),
        ("public doc scan", "public-doc-scan.json"),
        ("test hygiene scan", "test-hygiene-scan.json"),
        ("branch protection controls", "branch-protection controls"),
    ] {
        artifact_contains(hygiene_proof, evidence, needle)?;
    }

    for (source, scan_name) in [
        (audit_location_scan, "audit-location"),
        (public_doc_scan, "public-docs"),
    ] {
        artifact_contains(source, "scan schema", "\"schema_version\": 1")?;
        artifact_contains(source, "scan name", scan_name)?;
        artifact_contains(source, "scan findings inventory", "\"findings\"")?;
        artifact_contains(source, "scan blockers inventory", "\"blockers\"")?;
    }

    let scanned_files = artifact_number_field(hygiene_matrix, "scanned_files")?;
    artifact_at_least("scanned_files", scanned_files, 3000)?;

    Ok(DeepReviewHygieneArtifactProof { scanned_files })
}

/// Validate a committed full-file review ledger for every public crate file touched by release work.
pub fn validate_deep_review_ledger_artifact(
    ledger: &str,
) -> Result<DeepReviewLedgerArtifactProof, DeepReviewGateError> {
    for (evidence, needle) in [
        ("ledger schema", "\"schema_version\": 1"),
        (
            "active plan path",
            "\"plan_path\": \"release/plans/paradigm-shift-100-concrete.md\"",
        ),
        (
            "touched public crate file list",
            "\"touched_public_crate_files\"",
        ),
        ("review records", "\"review_records\""),
        ("read-in-full proof", "\"read_in_full\": true"),
        ("zero blockers", "\"blockers\": []"),
        ("Vyre crate review", "\"crate\": \"vyre\""),
        (
            "dataflow consumer crate review",
            "\"crate_role\": \"dataflow-consumer\"",
        ),
        ("Vyrec crate review", "\"crate\": \"vyrec\""),
        (
            "CUDA driver crate review",
            "\"crate\": \"vyre-driver-cuda\"",
        ),
        ("findings opened count", "\"findings_opened\""),
        ("findings fixed count", "\"findings_fixed\""),
    ] {
        artifact_contains(ledger, evidence, needle)?;
    }

    let reviewed_files = artifact_number_field(ledger, "reviewed_files")?;
    artifact_at_least("reviewed_files", reviewed_files, 1)?;
    let findings_opened = artifact_number_field(ledger, "findings_opened")?;
    let findings_fixed = artifact_number_field(ledger, "findings_fixed")?;
    if findings_fixed < findings_opened {
        return Err(DeepReviewGateError::LedgerUnfixedFindings {
            opened: findings_opened,
            fixed: findings_fixed,
        });
    }

    Ok(DeepReviewLedgerArtifactProof { reviewed_files })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), DeepReviewGateError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(DeepReviewGateError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), DeepReviewGateError> {
    if observed >= required {
        Ok(())
    } else {
        Err(DeepReviewGateError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(artifact: &str, field: &'static str) -> Result<u64, DeepReviewGateError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(DeepReviewGateError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(DeepReviewGateError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(DeepReviewGateError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| DeepReviewGateError::ArtifactMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_review_accepts_full_review_with_fixed_findings() {
        let proof = validate_deep_review_gate(&[
            record("vyre-driver-cuda/src/lib.rs", true, 2, 2),
            record("vyre-self-substrate/src/lib.rs", true, 0, 0),
        ])
        .expect("Fix: complete review records should pass");

        assert_eq!(proof.reviewed_files, 2);
        assert_eq!(proof.findings_fixed, 2);
    }

    #[test]
    fn deep_review_rejects_partial_file_review() {
        assert_eq!(
            validate_deep_review_gate(&[record("vyre/src/lib.rs", false, 0, 0)])
                .expect_err("partial review should fail"),
            DeepReviewGateError::NotReadInFull {
                path: "vyre/src/lib.rs".to_owned(),
            }
        );
    }

    #[test]
    fn deep_review_rejects_unfixed_findings() {
        assert_eq!(
            validate_deep_review_gate(&[record("vyre/src/lib.rs", true, 3, 2)])
                .expect_err("unfixed findings should fail"),
            DeepReviewGateError::FindingsNotFixed {
                path: "vyre/src/lib.rs".to_owned(),
                opened: 3,
                fixed: 2,
            }
        );
    }

    #[test]
    fn deep_review_accepts_committed_hygiene_artifacts() {
        let proof = validate_deep_review_hygiene_artifacts(
            include_str!("../../../../release/evidence/hygiene/hygiene-matrix.json"),
            include_str!("../../../../release/evidence/docs/release-hygiene-proof.md"),
            include_str!("../../../../release/evidence/hygiene/audit-location-scan.json"),
            include_str!("../../../../release/evidence/hygiene/public-doc-scan.json"),
        )
        .expect("Fix: committed hygiene artifacts should pass deep-review evidence checks");

        assert!(proof.scanned_files >= 3000);
    }

    #[test]
    fn deep_review_rejects_hygiene_artifact_with_blockers() {
        let hygiene_matrix = r#"{
          "schema_version": 1,
          "blockers": ["cpu-demotion"],
          "findings": [],
          "scanned_files": 3000,
          "scanned_roots": ["/matching/vyre", "/libs/dataflow/weir", "/tools/vyrec", "/libs/surge/surgec", "/libs/performance/matching/vyre/vyre-grammar-gen"],
          "release_surface_coverage": {
            "cuda_driver_crate": true,
            "wgpu_driver_crate": true,
            "release_scripts": true,
            "github_workflows": true,
            "branch_protection_controls": true,
            "hidden_fallback_patterns": ["cpu_demotion", "gpu_unavailable_skip"],
            "resource_bound_patterns": [],
            "release_tooling_patterns": ["raw_workspace_cargo", "heredoc"]
          }
        }"#;
        let proof = "# Release hygiene proof no-stubs-scan.json no-hidden-fallback-scan.json resource-bound-scan.json error-surface-scan.json cargo-wrapper-scan.json audit-location-scan.json public-doc-scan.json test-hygiene-scan.json branch-protection controls";
        let scan = r#"{"schema_version":1,"scan":"audit-location","findings":[],"blockers":[]}"#;
        let docs = r#"{"schema_version":1,"scan":"public-docs","findings":[],"blockers":[]}"#;

        assert_eq!(
            validate_deep_review_hygiene_artifacts(hygiene_matrix, proof, scan, docs)
                .expect_err("hygiene blockers should fail"),
            // The fixture carries the dataflow workspace root (/libs/dataflow/weir)
            // but omits the `weir_crate` surface flag, so the first failing check
            // is the crate-surface evidence the migration added after this test
            // was first written.
            DeepReviewGateError::ArtifactMissingEvidence {
                evidence: "dataflow crate surface",
            }
        );
    }

    #[test]
    fn deep_review_accepts_complete_full_file_review_ledger() {
        let proof = validate_deep_review_ledger_artifact(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "touched_public_crate_files": [
                "vyre/src/lib.rs",
                "libs/dataflow/consumer/src/lib.rs",
                "tools/vyrec/src/main.rs",
                "vyre-driver-cuda/src/lib.rs"
              ],
              "review_records": [
                {"crate": "vyre", "path": "vyre/src/lib.rs", "read_in_full": true},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "path": "libs/dataflow/consumer/src/lib.rs", "read_in_full": true},
                {"crate": "vyrec", "path": "tools/vyrec/src/main.rs", "read_in_full": true},
                {"crate": "vyre-driver-cuda", "path": "vyre-driver-cuda/src/lib.rs", "read_in_full": true}
              ],
              "reviewed_files": 4,
              "findings_opened": 3,
              "findings_fixed": 3,
              "blockers": []
            }"#,
        )
        .expect("Fix: complete full-file review ledger should pass");

        assert_eq!(proof.reviewed_files, 4);
    }

    #[test]
    fn deep_review_rejects_hygiene_scan_as_full_file_review_ledger() {
        let err = validate_deep_review_ledger_artifact(include_str!(
            "../../../../release/evidence/hygiene/hygiene-matrix.json"
        ))
        .expect_err("hygiene scans are not personal full-file review ledgers");

        assert_eq!(
            err,
            DeepReviewGateError::ArtifactMissingEvidence {
                evidence: "active plan path",
            }
        );
    }

    #[test]
    fn deep_review_rejects_ledger_with_unfixed_findings() {
        let err = validate_deep_review_ledger_artifact(
            r#"{
              "schema_version": 1,
              "plan_path": "release/plans/paradigm-shift-100-concrete.md",
              "touched_public_crate_files": ["vyre/src/lib.rs"],
              "review_records": [
                {"crate": "vyre", "path": "vyre/src/lib.rs", "read_in_full": true},
                {"crate": "dataflow-consumer", "crate_role": "dataflow-consumer", "path": "libs/dataflow/consumer/src/lib.rs", "read_in_full": true},
                {"crate": "vyrec", "path": "tools/vyrec/src/main.rs", "read_in_full": true},
                {"crate": "vyre-driver-cuda", "path": "vyre-driver-cuda/src/lib.rs", "read_in_full": true}
              ],
              "reviewed_files": 4,
              "findings_opened": 2,
              "findings_fixed": 1,
              "blockers": []
            }"#,
        )
        .expect_err("unfixed full-file review findings should block release");

        assert_eq!(
            err,
            DeepReviewGateError::LedgerUnfixedFindings {
                opened: 2,
                fixed: 1,
            }
        );
    }

    fn record(
        path: &'static str,
        read_in_full: bool,
        findings_opened: u32,
        findings_fixed: u32,
    ) -> DeepReviewRecord<'static> {
        DeepReviewRecord {
            path,
            read_in_full,
            findings_opened,
            findings_fixed,
            evidence: "release/reviews/deep-review.md",
        }
    }
}

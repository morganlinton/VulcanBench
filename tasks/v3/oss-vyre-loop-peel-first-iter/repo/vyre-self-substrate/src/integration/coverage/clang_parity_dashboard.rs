//! Vyrec clang-parity dashboard validation.

use std::collections::BTreeSet;

/// Compatibility class for one C frontend feature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClangParityStatus {
    /// Feature matches clang for the declared phase and corpus.
    Compatible,
    /// Feature has known partial parity.
    Partial,
    /// Feature currently fails parity and must be visible as a gap.
    Failing,
}

/// One clang-parity dashboard row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClangParityDashboardRow<'a> {
    /// Stable feature id.
    pub feature: &'a str,
    /// Frontend phase.
    pub phase: &'a str,
    /// Compatibility status.
    pub status: ClangParityStatus,
    /// Exact cargo_full test command.
    pub test_command: &'a str,
    /// Link or path to the test/gap artifact.
    pub evidence: &'a str,
}

/// Validated parity dashboard summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClangParityDashboardProof {
    /// Total row count.
    pub row_count: usize,
    /// Compatible row count.
    pub compatible_count: usize,
    /// Partial row count.
    pub partial_count: usize,
    /// Failing row count.
    pub failing_count: usize,
}

/// Committed clang parity evidence proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClangParityCommittedEvidenceProof {
    /// Linux C files covered by the CUDA parser corpus artifact.
    pub linux_file_count: u64,
    /// Linux C source bytes covered by the CUDA parser corpus artifact.
    pub linux_source_bytes: u64,
    /// Gap-suite files proving visible non-hidden parity gaps.
    pub gap_file_count: u64,
}

/// Dashboard validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClangParityDashboardError {
    /// Dashboard has no rows.
    EmptyDashboard,
    /// Duplicate feature/phase row.
    DuplicateRow {
        /// Feature.
        feature: String,
        /// Phase.
        phase: String,
    },
    /// Required metadata is empty.
    EmptyMetadata {
        /// Feature.
        feature: String,
        /// Field.
        field: &'static str,
    },
    /// Test command does not use cargo_full.
    TestCommandDoesNotUseCargoFull {
        /// Feature.
        feature: String,
        /// Command.
        command: String,
    },
    /// Dashboard lacks a required status class.
    MissingStatusClass {
        /// Missing class.
        status: ClangParityStatus,
    },
    /// Committed evidence is missing required parity data.
    EvidenceMissing {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed evidence is missing a numeric field.
    EvidenceMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed evidence missed a numeric threshold.
    EvidenceThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for ClangParityDashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDashboard => write!(
                f,
                "clang parity dashboard is empty. Fix: list compatible, partial, and failing frontend features with test evidence."
            ),
            Self::DuplicateRow { feature, phase } => write!(
                f,
                "clang parity dashboard duplicates feature `{feature}` phase `{phase}`. Fix: keep one authoritative row per feature phase."
            ),
            Self::EmptyMetadata { feature, field } => write!(
                f,
                "clang parity dashboard row `{feature}` has empty {field}. Fix: every row needs phase, status, test command, and evidence."
            ),
            Self::TestCommandDoesNotUseCargoFull { feature, command } => write!(
                f,
                "clang parity dashboard row `{feature}` uses `{command}` instead of ./cargo_full. Fix: make dashboard evidence reproducible through cargo_full."
            ),
            Self::MissingStatusClass { status } => write!(
                f,
                "clang parity dashboard is missing {status:?} rows. Fix: do not hide compatible, partial, or failing feature classes."
            ),
            Self::EvidenceMissing { evidence } => write!(
                f,
                "clang parity dashboard committed evidence is missing {evidence}. Fix: connect dashboard claims to CUDA corpus, frozen target, and visible gap evidence."
            ),
            Self::EvidenceMissingNumber { field } => write!(
                f,
                "clang parity dashboard committed evidence has no numeric {field}. Fix: record exact corpus and gap-suite counters."
            ),
            Self::EvidenceThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "clang parity dashboard committed evidence {field}={observed} missed required {required}. Fix: restore Linux parity corpus coverage or explicit gap evidence."
            ),
        }
    }
}

impl std::error::Error for ClangParityDashboardError {}

/// Validate a clang-parity dashboard artifact.
pub fn validate_clang_parity_dashboard(
    rows: &[ClangParityDashboardRow<'_>],
) -> Result<ClangParityDashboardProof, ClangParityDashboardError> {
    if rows.is_empty() {
        return Err(ClangParityDashboardError::EmptyDashboard);
    }
    let mut ids = BTreeSet::new();
    let mut compatible_count = 0_usize;
    let mut partial_count = 0_usize;
    let mut failing_count = 0_usize;

    for row in rows {
        for (field, value) in [
            ("feature", row.feature),
            ("phase", row.phase),
            ("test_command", row.test_command),
            ("evidence", row.evidence),
        ] {
            if value.trim().is_empty() {
                return Err(ClangParityDashboardError::EmptyMetadata {
                    feature: row.feature.to_owned(),
                    field,
                });
            }
        }
        if !ids.insert((row.feature, row.phase)) {
            return Err(ClangParityDashboardError::DuplicateRow {
                feature: row.feature.to_owned(),
                phase: row.phase.to_owned(),
            });
        }
        if !row.test_command.trim_start().starts_with("./cargo_full ") {
            return Err(ClangParityDashboardError::TestCommandDoesNotUseCargoFull {
                feature: row.feature.to_owned(),
                command: row.test_command.to_owned(),
            });
        }
        match row.status {
            ClangParityStatus::Compatible => compatible_count += 1,
            ClangParityStatus::Partial => partial_count += 1,
            ClangParityStatus::Failing => failing_count += 1,
        }
    }

    if compatible_count == 0 {
        return Err(ClangParityDashboardError::MissingStatusClass {
            status: ClangParityStatus::Compatible,
        });
    }
    if partial_count == 0 {
        return Err(ClangParityDashboardError::MissingStatusClass {
            status: ClangParityStatus::Partial,
        });
    }
    if failing_count == 0 {
        return Err(ClangParityDashboardError::MissingStatusClass {
            status: ClangParityStatus::Failing,
        });
    }

    Ok(ClangParityDashboardProof {
        row_count: rows.len(),
        compatible_count,
        partial_count,
        failing_count,
    })
}

/// Validate committed evidence backing the clang-parity dashboard.
pub fn validate_clang_parity_committed_evidence(
    linux_corpus_artifact: &str,
    target_manifest: &str,
    gap_suite_artifact: &str,
) -> Result<ClangParityCommittedEvidenceProof, ClangParityDashboardError> {
    for (evidence, needle) in [
        (
            "CUDA parser backend",
            "\"resident_vyre_parse_backend_id\": \"cuda\"",
        ),
        (
            "raw GPU syntax input",
            "\"resident_vyre_parse_input_mode\": \"raw_bytes_gpu_syntax\"",
        ),
        ("zero parser failures", "\"failed_files\": 0"),
        ("zero failure list", "\"failures\": []"),
        ("zero blocker list", "\"blockers\": []"),
        (
            "zero host token upload",
            "\"resident_vyre_parse_host_token_stream_upload_bytes\": 0",
        ),
    ] {
        evidence_contains(linux_corpus_artifact, evidence, needle)?;
    }
    for (evidence, needle) in [
        (
            "frozen parity target schema",
            "schema = \"vyrec.parity.target.v1\"",
        ),
        ("Linux lib/math target", "id = \"linux-lib-math-v6.8\""),
        ("GNU11 target language", "language = \"gnu11\""),
        (
            "semantic-analysis parity scope",
            "clang_parity_through = \"semantic-analysis\"",
        ),
        (
            "CPU oracle-only scope",
            "cpu_execution_allowed = \"oracle-only\"",
        ),
        ("GPU execution required", "gpu_execution_required = true"),
        (
            "zero unexplained mismatches gate",
            "zero_unexplained_parity_mismatches = true",
        ),
        (
            "resident GPU frontend gate",
            "resident_gpu_frontend_required = true",
        ),
    ] {
        evidence_contains(target_manifest, evidence, needle)?;
    }
    for (evidence, needle) in [
        ("gap suite marker", "\"suite\": \"gap\""),
        ("visible gap layer", "\"gap\""),
        ("Vyre gap files", "\"vyre_file_count\""),
        ("Dataflow consumer gap files", "\"file_count\""),
        ("Vyrec gap files", "\"vyrec_file_count\""),
        ("no gap-suite blockers", "\"blockers\": []"),
    ] {
        evidence_contains(gap_suite_artifact, evidence, needle)?;
    }

    let linux_file_count = evidence_number_field(linux_corpus_artifact, "total_files")?;
    let linux_source_bytes = evidence_number_field(linux_corpus_artifact, "total_source_bytes")?;
    let semantic_graph_bytes =
        evidence_number_field(linux_corpus_artifact, "total_semantic_graph_bytes")?;
    let covered_tokens = evidence_number_field(
        linux_corpus_artifact,
        "resident_vyre_parse_ast_covered_tokens",
    )?;
    let token_count =
        evidence_number_field(linux_corpus_artifact, "resident_vyre_parse_token_count")?;
    let gap_file_count = evidence_number_field(gap_suite_artifact, "file_count")?;
    let vyrec_gap_file_count = evidence_number_field(gap_suite_artifact, "vyrec_file_count")?;

    evidence_at_least("total_files", linux_file_count, 250)?;
    evidence_at_least("total_source_bytes", linux_source_bytes, 4 * 1024 * 1024)?;
    evidence_at_least(
        "total_semantic_graph_bytes",
        semantic_graph_bytes,
        1024 * 1024,
    )?;
    evidence_at_least("gap file_count", gap_file_count, 20)?;
    evidence_at_least("vyrec_file_count", vyrec_gap_file_count, 1)?;
    if covered_tokens != token_count {
        return Err(ClangParityDashboardError::EvidenceThresholdMiss {
            field: "resident_vyre_parse_ast_covered_tokens",
            observed: covered_tokens,
            required: token_count,
        });
    }

    Ok(ClangParityCommittedEvidenceProof {
        linux_file_count,
        linux_source_bytes,
        gap_file_count,
    })
}

fn evidence_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ClangParityDashboardError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ClangParityDashboardError::EvidenceMissing { evidence })
    }
}

fn evidence_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), ClangParityDashboardError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ClangParityDashboardError::EvidenceThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn evidence_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, ClangParityDashboardError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(ClangParityDashboardError::EvidenceMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(ClangParityDashboardError::EvidenceMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(ClangParityDashboardError::EvidenceMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| ClangParityDashboardError::EvidenceMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_dashboard_requires_all_status_classes() {
        let proof = validate_clang_parity_dashboard(&[
            row("lex-identifiers", ClangParityStatus::Compatible),
            row("macro-recursion", ClangParityStatus::Partial),
            row("semantic-usual-conversions", ClangParityStatus::Failing),
        ])
        .expect("Fix: dashboard with all classes should pass");

        assert_eq!(proof.row_count, 3);
        assert_eq!(proof.compatible_count, 1);
        assert_eq!(proof.partial_count, 1);
        assert_eq!(proof.failing_count, 1);
    }

    #[test]
    fn parity_dashboard_rejects_raw_cargo_commands() {
        let mut bad = row("bad", ClangParityStatus::Failing);
        bad.test_command = "cargo test";

        assert_eq!(
            validate_clang_parity_dashboard(&[bad]).expect_err("raw cargo should fail"),
            ClangParityDashboardError::TestCommandDoesNotUseCargoFull {
                feature: "bad".to_owned(),
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn parity_dashboard_rejects_hidden_failing_class() {
        assert_eq!(
            validate_clang_parity_dashboard(&[
                row("lex-identifiers", ClangParityStatus::Compatible),
                row("macro-recursion", ClangParityStatus::Partial),
            ])
            .expect_err("failing features must not be hidden"),
            ClangParityDashboardError::MissingStatusClass {
                status: ClangParityStatus::Failing,
            }
        );
    }

    #[test]
    fn parity_dashboard_accepts_committed_cuda_linux_and_gap_evidence() {
        let proof = validate_clang_parity_committed_evidence(
            include_str!("../../../../release/evidence/parser/c-parser-linux-subsystem.json"),
            include_str!("../../../../vyre-frontend-c/parity/linux_math_v6_8.toml"),
            include_str!("../../../../release/evidence/tests/gap-suite.json"),
        )
        .expect("Fix: committed clang parity evidence should pass");

        assert!(proof.linux_file_count >= 250);
        assert!(proof.linux_source_bytes >= 4 * 1024 * 1024);
        assert!(proof.gap_file_count >= 20);
    }

    #[test]
    fn parity_dashboard_rejects_cpu_or_unscoped_committed_evidence() {
        let linux = r#"{
          "resident_vyre_parse_backend_id": "cpu",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "failed_files": 0,
          "failures": [],
          "blockers": [],
          "resident_vyre_parse_host_token_stream_upload_bytes": 0,
          "total_files": 490,
          "total_source_bytes": 7394810,
          "total_semantic_graph_bytes": 3697526,
          "resident_vyre_parse_ast_covered_tokens": 10,
          "resident_vyre_parse_token_count": 10
        }"#;

        assert_eq!(
            validate_clang_parity_committed_evidence(
                linux,
                include_str!("../../../../vyre-frontend-c/parity/linux_math_v6_8.toml"),
                include_str!("../../../../release/evidence/tests/gap-suite.json"),
            )
            .expect_err("CPU parser evidence should fail dashboard validation"),
            ClangParityDashboardError::EvidenceMissing {
                evidence: "CUDA parser backend",
            }
        );
    }

    #[test]
    fn parity_dashboard_rejects_incomplete_token_coverage() {
        let linux = r#"{
          "resident_vyre_parse_backend_id": "cuda",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "failed_files": 0,
          "failures": [],
          "blockers": [],
          "resident_vyre_parse_host_token_stream_upload_bytes": 0,
          "total_files": 490,
          "total_source_bytes": 7394810,
          "total_semantic_graph_bytes": 3697526,
          "resident_vyre_parse_ast_covered_tokens": 9,
          "resident_vyre_parse_token_count": 10
        }"#;

        assert_eq!(
            validate_clang_parity_committed_evidence(
                linux,
                include_str!("../../../../vyre-frontend-c/parity/linux_math_v6_8.toml"),
                include_str!("../../../../release/evidence/tests/gap-suite.json"),
            )
            .expect_err("partial CUDA token coverage should fail dashboard validation"),
            ClangParityDashboardError::EvidenceThresholdMiss {
                field: "resident_vyre_parse_ast_covered_tokens",
                observed: 9,
                required: 10,
            }
        );
    }

    fn row(feature: &'static str, status: ClangParityStatus) -> ClangParityDashboardRow<'static> {
        ClangParityDashboardRow {
            feature,
            phase: "pre-lowering",
            status,
            test_command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-clang-dashboard.md",
        }
    }
}

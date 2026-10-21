//! Vyrec C semantic parity coverage validation.

use std::collections::BTreeSet;

/// Semantic category required for C/clang parity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticParityCategory {
    /// Declarations.
    Declarations,
    /// Typedef namespace behavior.
    Typedefs,
    /// Tag namespace behavior.
    TagNamespaces,
    /// Scope rules.
    Scopes,
    /// Linkage.
    Linkage,
    /// Storage class.
    StorageClass,
    /// Qualifiers.
    Qualifiers,
    /// Integer promotions.
    IntegerPromotions,
    /// Usual arithmetic conversions.
    UsualArithmeticConversions,
    /// Lvalue/rvalue rules.
    LvalueRules,
}

/// One semantic parity evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParityRecord<'a> {
    /// Semantic category.
    pub category: SemanticParityCategory,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Semantic parity proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParityCoverageProof {
    /// Number of covered categories.
    pub category_count: usize,
}

/// Committed Linux semantic artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParityLinuxArtifactProof {
    /// Linux C files covered by the artifact.
    pub total_files: u64,
    /// Linux C source bytes covered by the artifact.
    pub total_source_bytes: u64,
    /// GPU semantic graph bytes produced by the frontend.
    pub total_semantic_graph_bytes: u64,
    /// Median CUDA parser speedup over the baseline, scaled by 1000.
    pub speedup_x1000: u64,
}

/// Semantic parity validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticParityCoverageError {
    /// No records supplied.
    EmptyRecords,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Category.
        category: SemanticParityCategory,
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Category.
        category: SemanticParityCategory,
        /// Command.
        command: String,
    },
    /// Required category is missing.
    MissingCategory {
        /// Missing category.
        category: SemanticParityCategory,
    },
}

impl std::fmt::Display for SemanticParityCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "semantic parity coverage is empty. Fix: add clang parity evidence for every required C semantic category."
            ),
            Self::EmptyMetadata { category, field } => write!(
                f,
                "semantic parity record {category:?} has empty {field}. Fix: every category needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { category, command } => write!(
                f,
                "semantic parity record {category:?} uses `{command}` instead of ./cargo_full. Fix: run semantic parity through cargo_full."
            ),
            Self::MissingCategory { category } => write!(
                f,
                "semantic parity coverage is missing {category:?}. Fix: add explicit clang parity evidence for that semantic category."
            ),
        }
    }
}

impl std::error::Error for SemanticParityCoverageError {}

/// Committed Linux semantic artifact validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticParityLinuxArtifactError {
    /// Required literal field is missing.
    MissingField {
        /// Missing field.
        field: &'static str,
    },
    /// Required numeric field is missing or malformed.
    MissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Numeric field does not meet the release threshold.
    ThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required minimum or exact value.
        required: u64,
    },
}

impl std::fmt::Display for SemanticParityLinuxArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "semantic parity Linux artifact is missing {field}. Fix: commit real CUDA Linux parser semantic evidence, not synthetic coverage metadata."
            ),
            Self::MissingNumber { field } => write!(
                f,
                "semantic parity Linux artifact has no numeric {field}. Fix: record the exact release semantic counter."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "semantic parity Linux artifact {field}={observed} missed required {required}. Fix: improve the CUDA C frontend semantic path or lower the approved release target explicitly."
            ),
        }
    }
}

impl std::error::Error for SemanticParityLinuxArtifactError {}

const REQUIRED_CATEGORIES: &[SemanticParityCategory] = &[
    SemanticParityCategory::Declarations,
    SemanticParityCategory::Typedefs,
    SemanticParityCategory::TagNamespaces,
    SemanticParityCategory::Scopes,
    SemanticParityCategory::Linkage,
    SemanticParityCategory::StorageClass,
    SemanticParityCategory::Qualifiers,
    SemanticParityCategory::IntegerPromotions,
    SemanticParityCategory::UsualArithmeticConversions,
    SemanticParityCategory::LvalueRules,
];

/// Validate semantic parity coverage.
pub fn validate_semantic_parity_coverage(
    records: &[SemanticParityRecord<'_>],
) -> Result<SemanticParityCoverageProof, SemanticParityCoverageError> {
    if records.is_empty() {
        return Err(SemanticParityCoverageError::EmptyRecords);
    }
    let mut categories = BTreeSet::new();
    for record in records {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(SemanticParityCoverageError::EmptyMetadata {
                    category: record.category,
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(SemanticParityCoverageError::CommandDoesNotUseCargoFull {
                category: record.category,
                command: record.command.to_owned(),
            });
        }
        categories.insert(record.category);
    }
    for category in REQUIRED_CATEGORIES {
        if !categories.contains(category) {
            return Err(SemanticParityCoverageError::MissingCategory {
                category: *category,
            });
        }
    }
    Ok(SemanticParityCoverageProof {
        category_count: categories.len(),
    })
}

/// Validate the committed Linux C parser semantic artifact.
pub fn validate_semantic_parity_linux_artifact(
    artifact: &str,
) -> Result<SemanticParityLinuxArtifactProof, SemanticParityLinuxArtifactError> {
    artifact_contains(
        artifact,
        "CUDA parser backend",
        "\"resident_vyre_parse_backend_id\": \"cuda\"",
    )?;
    artifact_contains(
        artifact,
        "raw GPU syntax input",
        "\"resident_vyre_parse_input_mode\": \"raw_bytes_gpu_syntax\"",
    )?;
    artifact_contains(
        artifact,
        "prepared GPU syntax input",
        "\"resident_vyre_prepared_syntax_input\": true",
    )?;
    artifact_contains(
        artifact,
        "pipeline cache enabled",
        "\"resident_vyre_pipeline_cache_enabled\": true",
    )?;
    artifact_contains(artifact, "zero parser failures", "\"failures\": []")?;
    artifact_contains(artifact, "zero release blockers", "\"blockers\": []")?;

    let total_files = artifact_number_field(artifact, "total_files")?;
    let total_source_bytes = artifact_number_field(artifact, "total_source_bytes")?;
    let total_semantic_graph_bytes = artifact_number_field(artifact, "total_semantic_graph_bytes")?;
    let failed_files = artifact_number_field(artifact, "failed_files")?;
    let host_token_upload = artifact_number_field(
        artifact,
        "resident_vyre_parse_host_token_stream_upload_bytes",
    )?;
    let covered_tokens = artifact_number_field(artifact, "resident_vyre_parse_ast_covered_tokens")?;
    let token_count = artifact_number_field(artifact, "resident_vyre_parse_token_count")?;
    let gpu_dispatch_count =
        artifact_number_field(artifact, "resident_vyre_parse_gpu_dispatch_count")?;
    let host_submit_count =
        artifact_number_field(artifact, "resident_vyre_parse_host_submit_count")?;
    let resident_batch_dispatches =
        artifact_number_field(artifact, "resident_vyre_cuda_resident_batch_dispatches")?;
    let persistent_handle_dispatches =
        artifact_number_field(artifact, "resident_vyre_cuda_persistent_handle_dispatches")?;
    let static_param_dispatches =
        artifact_number_field(artifact, "resident_vyre_cuda_static_param_dispatches")?;
    let suppressed_ast_readback = artifact_number_field(
        artifact,
        "resident_vyre_parse_resident_ast_readback_suppressed_bytes",
    )?;
    let speedup_x1000 = artifact_number_field(
        artifact,
        "resident_vyre_vs_tree_sitter_median_speedup_x1000",
    )?;

    artifact_at_least("total_files", total_files, 250)?;
    artifact_at_least("total_source_bytes", total_source_bytes, 4 * 1024 * 1024)?;
    artifact_at_least(
        "total_semantic_graph_bytes",
        total_semantic_graph_bytes,
        1024 * 1024,
    )?;
    artifact_exact("failed_files", failed_files, 0)?;
    artifact_exact(
        "resident_vyre_parse_host_token_stream_upload_bytes",
        host_token_upload,
        0,
    )?;
    artifact_at_least(
        "resident_vyre_parse_gpu_dispatch_count",
        gpu_dispatch_count,
        1,
    )?;
    artifact_at_least(
        "resident_vyre_parse_host_submit_count",
        host_submit_count,
        1,
    )?;
    artifact_at_least(
        "resident_vyre_cuda_resident_batch_dispatches",
        resident_batch_dispatches,
        1,
    )?;
    artifact_at_least(
        "resident_vyre_cuda_persistent_handle_dispatches",
        persistent_handle_dispatches,
        1,
    )?;
    artifact_at_least(
        "resident_vyre_cuda_static_param_dispatches",
        static_param_dispatches,
        1,
    )?;
    artifact_at_least(
        "resident_vyre_parse_resident_ast_readback_suppressed_bytes",
        suppressed_ast_readback,
        total_source_bytes,
    )?;
    artifact_at_least(
        "resident_vyre_vs_tree_sitter_median_speedup_x1000",
        speedup_x1000,
        100_000,
    )?;
    if covered_tokens != token_count {
        return Err(SemanticParityLinuxArtifactError::ThresholdMiss {
            field: "resident_vyre_parse_ast_covered_tokens",
            observed: covered_tokens,
            required: token_count,
        });
    }

    Ok(SemanticParityLinuxArtifactProof {
        total_files,
        total_source_bytes,
        total_semantic_graph_bytes,
        speedup_x1000,
    })
}

fn artifact_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), SemanticParityLinuxArtifactError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(SemanticParityLinuxArtifactError::MissingField { field })
    }
}

fn artifact_exact(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), SemanticParityLinuxArtifactError> {
    if observed == required {
        Ok(())
    } else {
        Err(SemanticParityLinuxArtifactError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), SemanticParityLinuxArtifactError> {
    if observed >= required {
        Ok(())
    } else {
        Err(SemanticParityLinuxArtifactError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, SemanticParityLinuxArtifactError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(SemanticParityLinuxArtifactError::MissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(SemanticParityLinuxArtifactError::MissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(SemanticParityLinuxArtifactError::MissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| SemanticParityLinuxArtifactError::MissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_parity_coverage_accepts_all_required_categories() {
        let proof = validate_semantic_parity_coverage(&records())
            .expect("Fix: complete semantic parity coverage should pass");

        assert_eq!(proof.category_count, 10);
    }

    #[test]
    fn semantic_parity_coverage_rejects_missing_lvalue_rules() {
        let mut records = records();
        records.pop();

        assert_eq!(
            validate_semantic_parity_coverage(&records)
                .expect_err("missing lvalue rules should fail"),
            SemanticParityCoverageError::MissingCategory {
                category: SemanticParityCategory::LvalueRules,
            }
        );
    }

    #[test]
    fn semantic_parity_coverage_rejects_raw_cargo() {
        let mut records = records();
        records[0].command = "cargo test";

        assert_eq!(
            validate_semantic_parity_coverage(&records).expect_err("raw cargo should fail"),
            SemanticParityCoverageError::CommandDoesNotUseCargoFull {
                category: SemanticParityCategory::Declarations,
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn semantic_parity_linux_artifact_accepts_committed_cuda_linux_evidence() {
        let proof = validate_semantic_parity_linux_artifact(include_str!(
            "../../../../release/evidence/parser/c-parser-linux-subsystem.json"
        ))
        .expect("Fix: committed Linux CUDA semantic artifact should pass");

        assert!(proof.total_files >= 250);
        assert!(proof.total_source_bytes >= 4 * 1024 * 1024);
        assert!(proof.total_semantic_graph_bytes >= 1024 * 1024);
        assert!(proof.speedup_x1000 >= 100_000);
    }

    #[test]
    fn semantic_parity_linux_artifact_rejects_cpu_or_host_token_path() {
        let artifact = r#"{
          "resident_vyre_parse_backend_id": "cpu",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "resident_vyre_prepared_syntax_input": true,
          "resident_vyre_pipeline_cache_enabled": true,
          "failures": [],
          "blockers": [],
          "total_files": 490,
          "total_source_bytes": 7394810,
          "total_semantic_graph_bytes": 3697526,
          "failed_files": 0,
          "resident_vyre_parse_host_token_stream_upload_bytes": 1,
          "resident_vyre_parse_ast_covered_tokens": 10,
          "resident_vyre_parse_token_count": 10,
          "resident_vyre_parse_gpu_dispatch_count": 8,
          "resident_vyre_parse_host_submit_count": 1,
          "resident_vyre_cuda_resident_batch_dispatches": 10,
          "resident_vyre_cuda_persistent_handle_dispatches": 20,
          "resident_vyre_cuda_static_param_dispatches": 30,
          "resident_vyre_parse_resident_ast_readback_suppressed_bytes": 73953320,
          "resident_vyre_vs_tree_sitter_median_speedup_x1000": 3699018
        }"#;

        assert_eq!(
            validate_semantic_parity_linux_artifact(artifact)
                .expect_err("CPU semantic artifact should fail before counters are trusted"),
            SemanticParityLinuxArtifactError::MissingField {
                field: "CUDA parser backend",
            }
        );
    }

    #[test]
    fn semantic_parity_linux_artifact_rejects_incomplete_token_coverage() {
        let artifact = r#"{
          "resident_vyre_parse_backend_id": "cuda",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "resident_vyre_prepared_syntax_input": true,
          "resident_vyre_pipeline_cache_enabled": true,
          "failures": [],
          "blockers": [],
          "total_files": 490,
          "total_source_bytes": 7394810,
          "total_semantic_graph_bytes": 3697526,
          "failed_files": 0,
          "resident_vyre_parse_host_token_stream_upload_bytes": 0,
          "resident_vyre_parse_ast_covered_tokens": 9,
          "resident_vyre_parse_token_count": 10,
          "resident_vyre_parse_gpu_dispatch_count": 8,
          "resident_vyre_parse_host_submit_count": 1,
          "resident_vyre_cuda_resident_batch_dispatches": 10,
          "resident_vyre_cuda_persistent_handle_dispatches": 20,
          "resident_vyre_cuda_static_param_dispatches": 30,
          "resident_vyre_parse_resident_ast_readback_suppressed_bytes": 73953320,
          "resident_vyre_vs_tree_sitter_median_speedup_x1000": 3699018
        }"#;

        assert_eq!(
            validate_semantic_parity_linux_artifact(artifact)
                .expect_err("partial token coverage should fail"),
            SemanticParityLinuxArtifactError::ThresholdMiss {
                field: "resident_vyre_parse_ast_covered_tokens",
                observed: 9,
                required: 10,
            }
        );
    }

    fn records() -> Vec<SemanticParityRecord<'static>> {
        REQUIRED_CATEGORIES.iter().copied().map(record).collect()
    }

    fn record(category: SemanticParityCategory) -> SemanticParityRecord<'static> {
        SemanticParityRecord {
            category,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-semantic-parity.md",
        }
    }
}

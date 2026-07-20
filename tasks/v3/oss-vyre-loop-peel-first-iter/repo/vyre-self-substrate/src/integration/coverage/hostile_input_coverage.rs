//! Hostile input coverage validation.

use std::collections::BTreeSet;

/// Hostile input class required by the release plan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HostileInputClass {
    /// Invalid or undecodable bytes.
    InvalidBytes,
    /// Extreme parser/preprocessor nesting.
    ExtremeNesting,
    /// Include graph cycle.
    IncludeCycle,
    /// Recursive macro expansion.
    RecursiveMacro,
    /// Massive graph shape or fact set.
    MassiveGraph,
    /// Corrupted cache metadata.
    CorruptedCacheMetadata,
}

/// One hostile input evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostileInputCoverageRecord<'a> {
    /// Hostile input class.
    pub class: HostileInputClass,
    /// Owning module or crate.
    pub owner: &'a str,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path or test name.
    pub evidence: &'a str,
}

/// Hostile input coverage proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostileInputCoverageProof {
    /// Number of covered classes.
    pub class_count: usize,
}

/// Committed hostile-input artifact/source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostileInputArtifactProof {
    /// Adversarial suite file count.
    pub adversarial_file_count: u64,
}

/// Hostile input coverage validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostileInputCoverageError {
    /// No records supplied.
    EmptyRecords,
    /// Metadata is empty.
    EmptyMetadata {
        /// Field name.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Command.
        command: String,
    },
    /// A required hostile input class is missing.
    MissingClass {
        /// Missing class.
        class: HostileInputClass,
    },
    /// Committed hostile-input evidence is missing required proof.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed hostile-input evidence is missing a numeric field.
    ArtifactMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed hostile-input evidence missed a threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for HostileInputCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "hostile input coverage is empty. Fix: add evidence for invalid bytes, nesting, include cycles, recursive macros, massive graphs, and corrupted cache metadata."
            ),
            Self::EmptyMetadata { field } => write!(
                f,
                "hostile input coverage has empty {field}. Fix: record owner, command, and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { command } => write!(
                f,
                "hostile input coverage command `{command}` does not use ./cargo_full. Fix: run hostile tests through cargo_full."
            ),
            Self::MissingClass { class } => write!(
                f,
                "hostile input coverage is missing {class:?}. Fix: add an adversarial test for that class."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "hostile input committed evidence is missing {evidence}. Fix: preserve adversarial suite and source tests for invalid bytes, nesting, include cycles, recursive macros, massive graphs, and corrupted caches."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "hostile input committed evidence has no numeric {field}. Fix: record exact adversarial suite counters."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "hostile input committed evidence {field}={observed} missed required {required}. Fix: restore adversarial release coverage."
            ),
        }
    }
}

impl std::error::Error for HostileInputCoverageError {}

const REQUIRED_CLASSES: &[HostileInputClass] = &[
    HostileInputClass::InvalidBytes,
    HostileInputClass::ExtremeNesting,
    HostileInputClass::IncludeCycle,
    HostileInputClass::RecursiveMacro,
    HostileInputClass::MassiveGraph,
    HostileInputClass::CorruptedCacheMetadata,
];

/// Validate hostile input coverage.
pub fn validate_hostile_input_coverage(
    records: &[HostileInputCoverageRecord<'_>],
) -> Result<HostileInputCoverageProof, HostileInputCoverageError> {
    if records.is_empty() {
        return Err(HostileInputCoverageError::EmptyRecords);
    }
    let mut classes = BTreeSet::new();
    for record in records {
        for (field, value) in [
            ("owner", record.owner),
            ("command", record.command),
            ("evidence", record.evidence),
        ] {
            if value.trim().is_empty() {
                return Err(HostileInputCoverageError::EmptyMetadata { field });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(HostileInputCoverageError::CommandDoesNotUseCargoFull {
                command: record.command.to_owned(),
            });
        }
        classes.insert(record.class);
    }
    for class in REQUIRED_CLASSES {
        if !classes.contains(class) {
            return Err(HostileInputCoverageError::MissingClass { class: *class });
        }
    }
    Ok(HostileInputCoverageProof {
        class_count: classes.len(),
    })
}

/// Validate committed hostile input artifacts and source tests.
pub fn validate_hostile_input_artifacts(
    adversarial_suite: &str,
    lexer_diagnostics_source: &str,
    include_errors_source: &str,
    preprocessor_adversarial_source: &str,
    gpu_preprocess_adversarial_source: &str,
    cache_budget_source: &str,
) -> Result<HostileInputArtifactProof, HostileInputCoverageError> {
    for (evidence, needle) in [
        ("adversarial suite marker", "\"suite\": \"adversarial\""),
        ("zero blockers", "\"blockers\": []"),
        ("Vyre adversarial root", "/matching/vyre/"),
        (
            "Dataflow consumer adversarial coverage",
            "\"dataflow_consumer_file_count\": 43",
        ),
        ("Vyrec adversarial coverage", "\"vyrec_file_count\": 4"),
        (
            "hostile parser stream tests",
            "c_parser_hostile_malformed_stream_contracts",
        ),
        ("hostile full C parser tests", "c11_parser_hostile_full_c"),
        (
            "hostile Linux flow tests",
            "c_ast_linux_corpus_hostile_flow_and_pg_parity_contracts",
        ),
        ("engine cache adversarial tests", "engine_cache_adversarial"),
        ("corruption tests", "serial_envelope_corruption"),
    ] {
        artifact_contains(adversarial_suite, evidence, needle)?;
    }

    for (source, evidence, needle) in [
        (
            lexer_diagnostics_source,
            "invalid byte diagnostic",
            "invalid_escape",
        ),
        (
            lexer_diagnostics_source,
            "diagnostic rejects invalid escape",
            "invalid string or character escape",
        ),
        (
            include_errors_source,
            "include cycle A-B-A",
            "include_cycle_a_to_b_to_a_returns_error",
        ),
        (
            include_errors_source,
            "include self cycle",
            "include_self_cycle_returns_error",
        ),
        (
            preprocessor_adversarial_source,
            "nested conditional adversary",
            "deeply_nested_conditionals",
        ),
        (
            preprocessor_adversarial_source,
            "nested macro args",
            "function_macro_nested_parens_in_args",
        ),
        (
            gpu_preprocess_adversarial_source,
            "recursive macro adversary",
            "adversarial_recursive_object_macro_does_not_loop_or_cpu_fallback",
        ),
        (
            gpu_preprocess_adversarial_source,
            "hostile diagnostic",
            "hostile diagnostic",
        ),
        (
            cache_budget_source,
            "cache byte bound contract",
            "entry_bytes > max_bytes",
        ),
        (
            cache_budget_source,
            "cache overflow guard",
            // The committed cache-budget contract documents the overflow guard
            // as `saturating_add(entry_bytes)`: the byte accumulator pins to
            // usize::MAX on the (physically unreachable) overflow, which then
            // trips the `self.bytes > max_bytes` budget check. The earlier
            // `checked_add(entry_bytes)` needle predates that migration; the
            // contract artifact now asserts the saturating form, so the needle
            // tracks the artifact's actual overflow-guard evidence.
            "saturating_add(entry_bytes)",
        ),
    ] {
        artifact_contains(source, evidence, needle)?;
    }

    let adversarial_file_count = artifact_number_field(adversarial_suite, "file_count")?;
    artifact_at_least("adversarial file_count", adversarial_file_count, 100)?;
    let test_entrypoints = adversarial_suite
        .matches("\"has_test_entrypoint\": true")
        .count() as u64;
    artifact_at_least("adversarial test entrypoints", test_entrypoints, 50)?;

    Ok(HostileInputArtifactProof {
        adversarial_file_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), HostileInputCoverageError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(HostileInputCoverageError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), HostileInputCoverageError> {
    if observed >= required {
        Ok(())
    } else {
        Err(HostileInputCoverageError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, HostileInputCoverageError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(HostileInputCoverageError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(HostileInputCoverageError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(HostileInputCoverageError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| HostileInputCoverageError::ArtifactMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_input_coverage_accepts_all_required_classes() {
        let proof = validate_hostile_input_coverage(&records())
            .expect("Fix: complete hostile coverage should pass");

        assert_eq!(proof.class_count, 6);
    }

    #[test]
    fn hostile_input_coverage_rejects_missing_corrupted_cache() {
        let mut records = records();
        records.pop();

        assert_eq!(
            validate_hostile_input_coverage(&records)
                .expect_err("missing hostile class should fail"),
            HostileInputCoverageError::MissingClass {
                class: HostileInputClass::CorruptedCacheMetadata,
            }
        );
    }

    #[test]
    fn hostile_input_coverage_rejects_raw_cargo() {
        let mut records = records();
        records[0].command = "cargo test";

        assert_eq!(
            validate_hostile_input_coverage(&records).expect_err("raw cargo should fail"),
            HostileInputCoverageError::CommandDoesNotUseCargoFull {
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn hostile_input_coverage_accepts_committed_adversarial_artifacts() {
        let proof = validate_hostile_input_artifacts(
            include_str!("../../../../release/evidence/tests/adversarial-suite.json"),
            include_str!("../../../../vyre-frontend-c/tests/c11_lexer_diagnostics.rs"),
            include_str!("../../../../vyre-frontend-c/tests/preprocessor_include_errors.rs"),
            include_str!("../../../../vyre-frontend-c/tests/preprocessor_adversarial.rs"),
            include_str!("../../../../vyre-frontend-c/tests/gpu_preprocess_adversarial.rs"),
            include_str!(
                "../../../../vyre-frontend-c/tests/frontend_pipeline_cache_budget_contract.rs"
            ),
        )
        .expect("Fix: committed hostile input evidence should pass");

        assert!(proof.adversarial_file_count >= 100);
    }

    #[test]
    fn hostile_input_coverage_rejects_missing_recursive_macro_source() {
        let err = validate_hostile_input_artifacts(
            include_str!("../../../../release/evidence/tests/adversarial-suite.json"),
            include_str!("../../../../vyre-frontend-c/tests/c11_lexer_diagnostics.rs"),
            include_str!("../../../../vyre-frontend-c/tests/preprocessor_include_errors.rs"),
            include_str!("../../../../vyre-frontend-c/tests/preprocessor_adversarial.rs"),
            "fn unrelated() {}",
            include_str!(
                "../../../../vyre-frontend-c/tests/frontend_pipeline_cache_budget_contract.rs"
            ),
        )
        .expect_err("missing recursive macro source should fail");

        assert_eq!(
            err,
            HostileInputCoverageError::ArtifactMissingEvidence {
                evidence: "recursive macro adversary",
            }
        );
    }

    fn records() -> Vec<HostileInputCoverageRecord<'static>> {
        vec![
            record(HostileInputClass::InvalidBytes),
            record(HostileInputClass::ExtremeNesting),
            record(HostileInputClass::IncludeCycle),
            record(HostileInputClass::RecursiveMacro),
            record(HostileInputClass::MassiveGraph),
            record(HostileInputClass::CorruptedCacheMetadata),
        ]
    }

    fn record(class: HostileInputClass) -> HostileInputCoverageRecord<'static> {
        HostileInputCoverageRecord {
            class,
            owner: "vyrec/dataflow",
            command: "./cargo_full test -j1 -p vyre-self-substrate",
            evidence: "release/hostile-input.md",
        }
    }
}

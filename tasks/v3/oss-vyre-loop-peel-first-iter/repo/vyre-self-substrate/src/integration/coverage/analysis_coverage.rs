//! Dataflow analysis release coverage validation.

use std::collections::{BTreeMap, BTreeSet};

/// Dataflow analysis required by the release plan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataflowAnalysis {
    /// Reaching definitions.
    ReachingDefinitions,
    /// Liveness.
    Liveness,
    /// Points-to analysis.
    PointsTo,
    /// Program slicing.
    Slicing,
    /// IFDS/IDE-style solve.
    Ifds,
}

/// Release evidence required for every Dataflow analysis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataflowAnalysisEvidence {
    /// Caller-owned output scratch API.
    CallerOwnedScratch,
    /// Resident graph/frontier/result buffers.
    Residency,
    /// Monotonicity and convergence properties.
    MonotonicityConvergence,
    /// Lattice join associativity/idempotence where applicable.
    LatticeLaws,
    /// Adversarial graph suite.
    AdversarialGraphs,
    /// GPU residency/allocation regression evidence.
    GpuAllocationEvidence,
}

/// One Dataflow analysis coverage record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataflowAnalysisCoverageRecord<'a> {
    /// Analysis.
    pub analysis: DataflowAnalysis,
    /// Evidence kind.
    pub evidence_kind: DataflowAnalysisEvidence,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path.
    pub evidence: &'a str,
}

/// Dataflow analysis coverage proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataflowAnalysisCoverageProof {
    /// Analysis count.
    pub analysis_count: usize,
    /// Evidence record count.
    pub record_count: usize,
}

/// Validated committed Dataflow analysis artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataflowAnalysisArtifactProof {
    /// Public Dataflow analysis API rows.
    pub analysis_count: u64,
    /// Integration test rows.
    pub integration_test_count: u64,
    /// Total integration assertions.
    pub assertion_count: u64,
}

/// Dataflow analysis coverage validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataflowAnalysisCoverageError {
    /// Records are empty.
    EmptyRecords,
    /// Metadata is empty.
    EmptyMetadata {
        /// Analysis.
        analysis: DataflowAnalysis,
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Analysis.
        analysis: DataflowAnalysis,
        /// Command.
        command: String,
    },
    /// Required analysis is missing.
    MissingAnalysis {
        /// Analysis.
        analysis: DataflowAnalysis,
    },
    /// Required evidence is missing for an analysis.
    MissingEvidence {
        /// Analysis.
        analysis: DataflowAnalysis,
        /// Evidence kind.
        evidence_kind: DataflowAnalysisEvidence,
    },
    /// Committed Dataflow consumer artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed Dataflow consumer artifact numeric field is missing.
    ArtifactMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed Dataflow consumer artifact threshold was missed.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for DataflowAnalysisCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "Dataflow analysis coverage is empty. Fix: add release evidence for reaching, liveness, points-to, slicing, and IFDS."
            ),
            Self::EmptyMetadata { analysis, field } => write!(
                f,
                "Dataflow analysis coverage {analysis:?} has empty {field}. Fix: every record needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { analysis, command } => write!(
                f,
                "Dataflow analysis coverage {analysis:?} uses `{command}` instead of ./cargo_full. Fix: run Dataflow consumer evidence through cargo_full."
            ),
            Self::MissingAnalysis { analysis } => write!(
                f,
                "Dataflow analysis coverage is missing {analysis:?}. Fix: add evidence for that required analysis."
            ),
            Self::MissingEvidence {
                analysis,
                evidence_kind,
            } => write!(
                f,
                "Dataflow analysis coverage {analysis:?} is missing {evidence_kind:?}. Fix: add that release evidence before shipping."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "Dataflow analysis artifact is missing {evidence}. Fix: commit Dataflow consumer API, property, parity, adversarial, benchmark, fuzz, gap, and integration evidence."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "Dataflow analysis artifact has no numeric {field}. Fix: record exact Dataflow analysis coverage counters."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "Dataflow analysis artifact {field}={observed} missed required {required}. Fix: expand Dataflow analysis/test coverage before release."
            ),
        }
    }
}

impl std::error::Error for DataflowAnalysisCoverageError {}

const REQUIRED_ANALYSES: &[DataflowAnalysis] = &[
    DataflowAnalysis::ReachingDefinitions,
    DataflowAnalysis::Liveness,
    DataflowAnalysis::PointsTo,
    DataflowAnalysis::Slicing,
    DataflowAnalysis::Ifds,
];

const REQUIRED_EVIDENCE: &[DataflowAnalysisEvidence] = &[
    DataflowAnalysisEvidence::CallerOwnedScratch,
    DataflowAnalysisEvidence::Residency,
    DataflowAnalysisEvidence::MonotonicityConvergence,
    DataflowAnalysisEvidence::LatticeLaws,
    DataflowAnalysisEvidence::AdversarialGraphs,
    DataflowAnalysisEvidence::GpuAllocationEvidence,
];

/// Validate Dataflow analysis release coverage.
pub fn validate_analysis_coverage(
    records: &[DataflowAnalysisCoverageRecord<'_>],
) -> Result<DataflowAnalysisCoverageProof, DataflowAnalysisCoverageError> {
    if records.is_empty() {
        return Err(DataflowAnalysisCoverageError::EmptyRecords);
    }
    let mut by_analysis: BTreeMap<DataflowAnalysis, BTreeSet<DataflowAnalysisEvidence>> =
        BTreeMap::new();
    for record in records {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(DataflowAnalysisCoverageError::EmptyMetadata {
                    analysis: record.analysis,
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(DataflowAnalysisCoverageError::CommandDoesNotUseCargoFull {
                analysis: record.analysis,
                command: record.command.to_owned(),
            });
        }
        by_analysis
            .entry(record.analysis)
            .or_default()
            .insert(record.evidence_kind);
    }

    for analysis in REQUIRED_ANALYSES {
        let Some(evidence) = by_analysis.get(analysis) else {
            return Err(DataflowAnalysisCoverageError::MissingAnalysis {
                analysis: *analysis,
            });
        };
        for evidence_kind in REQUIRED_EVIDENCE {
            if !evidence.contains(evidence_kind) {
                return Err(DataflowAnalysisCoverageError::MissingEvidence {
                    analysis: *analysis,
                    evidence_kind: *evidence_kind,
                });
            }
        }
    }

    Ok(DataflowAnalysisCoverageProof {
        analysis_count: by_analysis.len(),
        record_count: records.len(),
    })
}

/// Validate committed Dataflow consumer API and integration-test artifacts.
pub fn validate_committed_analysis_artifacts(
    api_matrix: &str,
    integration_tests: &str,
) -> Result<DataflowAnalysisArtifactProof, DataflowAnalysisCoverageError> {
    for (artifact, evidence, needle) in [
        (api_matrix, "API matrix schema", "\"schema_version\": 2"),
        (
            api_matrix,
            "zero missing API items",
            "\"missing_api_item_count\": 0",
        ),
        (api_matrix, "zero blockers", "\"blockers\": []"),
        (api_matrix, "SSA API", "\"id\": \"ssa\""),
        (api_matrix, "reaching API", "\"id\": \"reaching\""),
        (api_matrix, "liveness API", "\"id\": \"live\""),
        (api_matrix, "points-to API", "\"id\": \"points_to\""),
        (api_matrix, "slice API", "\"id\": \"slice\""),
        (api_matrix, "IFDS API", "\"id\": \"ifds\""),
        (api_matrix, "callgraph API", "\"id\": \"callgraph\""),
        (api_matrix, "public exports", "\"public_exported\": true"),
        (api_matrix, "standalone examples", "\"standalone_examples\""),
        (
            api_matrix,
            "serde feature evidence",
            "\"standalone_serde_feature_guard_count\"",
        ),
        (
            integration_tests,
            "integration schema",
            "\"schema_version\": 2",
        ),
        (
            integration_tests,
            "integration zero blockers",
            "\"blockers\": []",
        ),
        (
            integration_tests,
            "adversarial tests",
            "\"id\": \"adversarial_oracles\"",
        ),
        (
            integration_tests,
            "parity tests",
            "\"id\": \"parity_exact_primitives\"",
        ),
        (
            integration_tests,
            "points-to property tests",
            "\"id\": \"property_points_to\"",
        ),
        (
            integration_tests,
            "IFDS property tests",
            "\"id\": \"property_ifds\"",
        ),
        (
            integration_tests,
            "slice property tests",
            "\"id\": \"property_slice\"",
        ),
        (
            integration_tests,
            "fuzz tests",
            "\"id\": \"fuzz_bitset_oracles\"",
        ),
        (
            integration_tests,
            "gap tests",
            "\"id\": \"gap_bitset_oracle_edges\"",
        ),
        (
            integration_tests,
            "performance tests",
            "\"id\": \"perf_oracle\"",
        ),
        (integration_tests, "scale tests", "\"id\": \"scale_oracle\""),
        (
            integration_tests,
            "test entrypoints",
            "\"has_test_entrypoint\": true",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let required_api_items = number_field(api_matrix, "required_api_item_count")?;
    let inventory_registered = number_field(api_matrix, "inventory_registered_count")?;
    let property_tests = number_field(api_matrix, "property_test_count")?;
    let parity_tests = number_field(api_matrix, "parity_test_count")?;
    let perf_tests = number_field(api_matrix, "perf_test_count")?;
    let fuzz_tests = number_field(api_matrix, "fuzz_test_count")?;
    let gap_tests = number_field(api_matrix, "gap_test_count")?;
    let standalone_examples = number_field(api_matrix, "standalone_example_count")?;
    let analysis_count = count_occurrences(api_matrix, "\"id\": ");
    let integration_test_count = count_occurrences(integration_tests, "\"id\": ");
    let assertion_count = sum_assertion_counts(integration_tests);

    require_at_least("required_api_item_count", required_api_items, 100)?;
    require_at_least("inventory_registered_count", inventory_registered, 20)?;
    require_at_least("property_test_count", property_tests, 10)?;
    require_at_least("parity_test_count", parity_tests, 4)?;
    require_at_least("perf_test_count", perf_tests, 2)?;
    require_at_least("fuzz_test_count", fuzz_tests, 1)?;
    require_at_least("gap_test_count", gap_tests, 1)?;
    require_at_least("standalone_example_count", standalone_examples, 2)?;
    require_at_least("analysis row count", analysis_count, 20)?;
    require_at_least("integration test row count", integration_test_count, 40)?;
    require_at_least("integration assertion count", assertion_count, 400)?;

    Ok(DataflowAnalysisArtifactProof {
        analysis_count,
        integration_test_count,
        assertion_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), DataflowAnalysisCoverageError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(DataflowAnalysisCoverageError::ArtifactMissingEvidence { evidence })
    }
}

fn require_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), DataflowAnalysisCoverageError> {
    if observed >= required {
        Ok(())
    } else {
        Err(DataflowAnalysisCoverageError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn number_field(artifact: &str, field: &'static str) -> Result<u64, DataflowAnalysisCoverageError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(DataflowAnalysisCoverageError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(DataflowAnalysisCoverageError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(DataflowAnalysisCoverageError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| DataflowAnalysisCoverageError::ArtifactMissingNumber { field })
}

fn count_occurrences(haystack: &str, needle: &str) -> u64 {
    haystack.matches(needle).count() as u64
}

fn sum_assertion_counts(artifact: &str) -> u64 {
    let mut total = 0_u64;
    let mut rest = artifact;
    while let Some(index) = rest.find("\"assertion_count\"") {
        rest = &rest[index + "\"assertion_count\"".len()..];
        let Some(colon) = rest.find(':') else {
            break;
        };
        let after_colon = rest[colon + 1..].trim_start();
        let digits = after_colon
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if let Ok(value) = digits.parse::<u64>() {
            total = total.saturating_add(value);
        }
        rest = after_colon;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_coverage_accepts_all_required_evidence() {
        let proof = validate_analysis_coverage(&records())
            .expect("Fix: complete Dataflow analysis coverage should pass");

        assert_eq!(proof.analysis_count, 5);
        assert_eq!(proof.record_count, 30);
    }

    #[test]
    fn analysis_coverage_rejects_missing_ifds() {
        let records: Vec<_> = records()
            .into_iter()
            .filter(|record| record.analysis != DataflowAnalysis::Ifds)
            .collect();

        assert_eq!(
            validate_analysis_coverage(&records).expect_err("missing IFDS should fail"),
            DataflowAnalysisCoverageError::MissingAnalysis {
                analysis: DataflowAnalysis::Ifds,
            }
        );
    }

    #[test]
    fn analysis_coverage_rejects_missing_evidence_and_raw_cargo() {
        let mut missing_evidence_records = records();
        missing_evidence_records.retain(|record| {
            !(record.analysis == DataflowAnalysis::Liveness
                && record.evidence_kind == DataflowAnalysisEvidence::GpuAllocationEvidence)
        });
        assert_eq!(
            validate_analysis_coverage(&missing_evidence_records)
                .expect_err("missing evidence should fail"),
            DataflowAnalysisCoverageError::MissingEvidence {
                analysis: DataflowAnalysis::Liveness,
                evidence_kind: DataflowAnalysisEvidence::GpuAllocationEvidence,
            }
        );

        let mut raw_cargo_records = records();
        raw_cargo_records[0].command = "cargo test";
        assert_eq!(
            validate_analysis_coverage(&raw_cargo_records).expect_err("raw cargo should fail"),
            DataflowAnalysisCoverageError::CommandDoesNotUseCargoFull {
                analysis: DataflowAnalysis::ReachingDefinitions,
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn analysis_coverage_accepts_committed_api_and_integration_artifacts() {
        let proof = validate_committed_analysis_artifacts(
            include_str!("../../../../release/evidence/dataflow/analysis-api-matrix.json"),
            include_str!("../../../../release/evidence/dataflow/integration-tests.json"),
        )
        .expect("Fix: committed Dataflow analysis artifacts should pass");

        assert!(proof.analysis_count >= 20);
        assert!(proof.integration_test_count >= 40);
        assert!(proof.assertion_count >= 400);
    }

    #[test]
    fn analysis_coverage_rejects_partial_committed_artifacts() {
        let err = validate_committed_analysis_artifacts(
            r#"{"schema_version": 2, "missing_api_item_count": 0, "blockers": [], "required_api_item_count": 1, "inventory_registered_count": 1, "property_test_count": 1, "parity_test_count": 1, "perf_test_count": 0, "fuzz_test_count": 0, "gap_test_count": 0, "standalone_example_count": 0, "analyses": [{"id": "ssa", "public_exported": true}], "standalone_examples": [], "standalone_serde_feature_guard_count": 0}"#,
            include_str!("../../../../release/evidence/dataflow/integration-tests.json"),
        )
        .expect_err("partial Dataflow consumer API artifact should fail");

        assert_eq!(
            err,
            DataflowAnalysisCoverageError::ArtifactMissingEvidence {
                evidence: "reaching API",
            }
        );
    }

    fn records() -> Vec<DataflowAnalysisCoverageRecord<'static>> {
        let mut records = Vec::new();
        for analysis in REQUIRED_ANALYSES {
            for evidence_kind in REQUIRED_EVIDENCE {
                records.push(DataflowAnalysisCoverageRecord {
                    analysis: *analysis,
                    evidence_kind: *evidence_kind,
                    command: "./cargo_full test -j1 -p dataflow",
                    evidence: "release/dataflow/analysis-coverage.md",
                });
            }
        }
        records
    }
}

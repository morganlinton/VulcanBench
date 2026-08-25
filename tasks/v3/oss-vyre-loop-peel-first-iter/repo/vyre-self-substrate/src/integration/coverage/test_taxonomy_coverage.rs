//! Release test taxonomy coverage validation.

use std::collections::{BTreeMap, BTreeSet};

/// Test taxonomy required for every major release module.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TestTaxonomy {
    /// Normal-condition unit tests.
    Unit,
    /// Hostile or malformed input tests.
    Adversarial,
    /// Invariant/property tests.
    Property,
    /// Performance threshold tests or benchmarks.
    Benchmark,
    /// Fuzz or arbitrary-input tests.
    Fuzz,
    /// Explicit missing-capability/gap tests.
    Gap,
}

/// One test coverage record for a module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestCoverageRecord<'a> {
    /// Major module name.
    pub module: &'a str,
    /// Test taxonomy covered.
    pub taxonomy: TestTaxonomy,
    /// Exact command that runs the evidence.
    pub command: &'a str,
    /// Evidence path or test name.
    pub evidence: &'a str,
}

/// Test coverage proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestTaxonomyCoverageProof {
    /// Major modules covered.
    pub module_count: usize,
    /// Coverage records accepted.
    pub record_count: usize,
}

/// Committed release test-suite artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestTaxonomySuiteArtifactProof {
    /// Number of accepted suite artifacts.
    pub suite_count: usize,
    /// Total release test files across accepted suite artifacts.
    pub total_file_count: u64,
}

/// Test coverage validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestTaxonomyCoverageError {
    /// No coverage records were supplied.
    EmptyRecords,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Module name.
        module: String,
        /// Field name.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Module name.
        module: String,
        /// Command.
        command: String,
    },
    /// A module is missing a required test taxonomy.
    MissingTaxonomy {
        /// Module name.
        module: String,
        /// Missing taxonomy.
        taxonomy: TestTaxonomy,
    },
    /// Committed test-suite artifact is missing required evidence.
    SuiteArtifactMissingEvidence {
        /// Suite name.
        suite: &'static str,
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed test-suite artifact is missing a numeric field.
    SuiteArtifactMissingNumber {
        /// Suite name.
        suite: &'static str,
        /// Missing field.
        field: &'static str,
    },
    /// Committed test-suite artifact missed a threshold.
    SuiteArtifactThresholdMiss {
        /// Suite name.
        suite: &'static str,
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for TestTaxonomyCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "test taxonomy coverage is empty. Fix: every major module needs unit, adversarial, property, benchmark, fuzz, and gap evidence."
            ),
            Self::EmptyMetadata { module, field } => write!(
                f,
                "test taxonomy coverage record for `{module}` has empty {field}. Fix: record module, command, and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { module, command } => write!(
                f,
                "test taxonomy coverage record for `{module}` uses `{command}` instead of ./cargo_full. Fix: run release tests through cargo_full."
            ),
            Self::MissingTaxonomy { module, taxonomy } => write!(
                f,
                "test taxonomy coverage for `{module}` is missing {taxonomy:?}. Fix: add that test class before release."
            ),
            Self::SuiteArtifactMissingEvidence { suite, evidence } => write!(
                f,
                "test taxonomy suite artifact `{suite}` is missing {evidence}. Fix: commit real release test-suite evidence with zero blockers."
            ),
            Self::SuiteArtifactMissingNumber { suite, field } => write!(
                f,
                "test taxonomy suite artifact `{suite}` has no numeric {field}. Fix: record exact suite counters."
            ),
            Self::SuiteArtifactThresholdMiss {
                suite,
                field,
                observed,
                required,
            } => write!(
                f,
                "test taxonomy suite artifact `{suite}` {field}={observed} missed required {required}. Fix: restore platform/dataflow/frontend test coverage for that taxonomy."
            ),
        }
    }
}

impl std::error::Error for TestTaxonomyCoverageError {}

const REQUIRED_TAXONOMIES: &[TestTaxonomy] = &[
    TestTaxonomy::Unit,
    TestTaxonomy::Adversarial,
    TestTaxonomy::Property,
    TestTaxonomy::Benchmark,
    TestTaxonomy::Fuzz,
    TestTaxonomy::Gap,
];

/// Validate release test taxonomy coverage for each major module.
pub fn validate_test_taxonomy_coverage(
    records: &[TestCoverageRecord<'_>],
) -> Result<TestTaxonomyCoverageProof, TestTaxonomyCoverageError> {
    if records.is_empty() {
        return Err(TestTaxonomyCoverageError::EmptyRecords);
    }

    let mut by_module: BTreeMap<&str, BTreeSet<TestTaxonomy>> = BTreeMap::new();
    for record in records {
        for (field, value) in [
            ("module", record.module),
            ("command", record.command),
            ("evidence", record.evidence),
        ] {
            if value.trim().is_empty() {
                return Err(TestTaxonomyCoverageError::EmptyMetadata {
                    module: record.module.to_owned(),
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(TestTaxonomyCoverageError::CommandDoesNotUseCargoFull {
                module: record.module.to_owned(),
                command: record.command.to_owned(),
            });
        }
        by_module
            .entry(record.module)
            .or_default()
            .insert(record.taxonomy);
    }

    for (module, taxonomies) in &by_module {
        for taxonomy in REQUIRED_TAXONOMIES {
            if !taxonomies.contains(taxonomy) {
                return Err(TestTaxonomyCoverageError::MissingTaxonomy {
                    module: (*module).to_owned(),
                    taxonomy: *taxonomy,
                });
            }
        }
    }

    Ok(TestTaxonomyCoverageProof {
        module_count: by_module.len(),
        record_count: records.len(),
    })
}

/// Validate committed test-suite artifacts for all required taxonomies.
pub fn validate_test_taxonomy_suite_artifacts(
    suites: &[(&'static str, &'static str, u64, u64, u64, u64)],
) -> Result<TestTaxonomySuiteArtifactProof, TestTaxonomyCoverageError> {
    if suites.is_empty() {
        return Err(TestTaxonomyCoverageError::EmptyRecords);
    }

    let mut names = BTreeSet::new();
    let mut total_file_count = 0_u64;
    for (suite, artifact, min_total, min_vyre, min_dataflow_consumer, min_vyrec) in suites {
        if !names.insert(*suite) {
            return Err(TestTaxonomyCoverageError::SuiteArtifactMissingEvidence {
                suite,
                evidence: "unique suite name",
            });
        }
        suite_contains(
            suite,
            artifact,
            "suite marker",
            &format!("\"suite\": \"{suite}\""),
        )?;
        suite_contains(suite, artifact, "zero blockers", "\"blockers\": []")?;
        suite_contains(suite, artifact, "files list", "\"files\"")?;
        suite_contains(suite, artifact, "Vyre test root", "/matching/vyre/")?;
        suite_contains(suite, artifact, "assertion counters", "\"assertion_count\"")?;

        let file_count = suite_number_field(suite, artifact, "file_count")?;
        let vyre_file_count = suite_number_field(suite, artifact, "vyre_file_count")?;
        let dataflow_consumer_file_count =
            suite_number_field(suite, artifact, "dataflow_consumer_file_count")?;
        let vyrec_file_count = suite_number_field(suite, artifact, "vyrec_file_count")?;
        let test_entrypoints = artifact.matches("\"has_test_entrypoint\": true").count() as u64;

        suite_at_least(suite, "file_count", file_count, *min_total)?;
        suite_at_least(suite, "vyre_file_count", vyre_file_count, *min_vyre)?;
        suite_at_least(
            suite,
            "dataflow_consumer_file_count",
            dataflow_consumer_file_count,
            *min_dataflow_consumer,
        )?;
        suite_at_least(suite, "vyrec_file_count", vyrec_file_count, *min_vyrec)?;
        suite_at_least(suite, "has_test_entrypoint=true", test_entrypoints, 1)?;
        total_file_count = total_file_count.saturating_add(file_count);
    }

    for required in [
        "unit",
        "adversarial",
        "property",
        "benchmark",
        "fuzz",
        "gap",
    ] {
        if !names.contains(required) {
            return Err(TestTaxonomyCoverageError::SuiteArtifactMissingEvidence {
                suite: required,
                evidence: "required taxonomy suite",
            });
        }
    }

    Ok(TestTaxonomySuiteArtifactProof {
        suite_count: suites.len(),
        total_file_count,
    })
}

fn suite_contains(
    suite: &'static str,
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), TestTaxonomyCoverageError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(TestTaxonomyCoverageError::SuiteArtifactMissingEvidence { suite, evidence })
    }
}

fn suite_at_least(
    suite: &'static str,
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), TestTaxonomyCoverageError> {
    if observed >= required {
        Ok(())
    } else {
        Err(TestTaxonomyCoverageError::SuiteArtifactThresholdMiss {
            suite,
            field,
            observed,
            required,
        })
    }
}

fn suite_number_field(
    suite: &'static str,
    artifact: &str,
    field: &'static str,
) -> Result<u64, TestTaxonomyCoverageError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(TestTaxonomyCoverageError::SuiteArtifactMissingNumber { suite, field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(TestTaxonomyCoverageError::SuiteArtifactMissingNumber { suite, field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(TestTaxonomyCoverageError::SuiteArtifactMissingNumber { suite, field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| TestTaxonomyCoverageError::SuiteArtifactMissingNumber { suite, field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_coverage_accepts_all_required_test_classes() {
        let proof = validate_test_taxonomy_coverage(&records("cuda-megakernel"))
            .expect("Fix: complete test taxonomy should pass");

        assert_eq!(proof.module_count, 1);
        assert_eq!(proof.record_count, 6);
    }

    #[test]
    fn taxonomy_coverage_rejects_missing_gap_tests() {
        let mut records = records("vyrec-parser");
        records.pop();

        assert_eq!(
            validate_test_taxonomy_coverage(&records)
                .expect_err("missing gap taxonomy should fail"),
            TestTaxonomyCoverageError::MissingTaxonomy {
                module: "vyrec-parser".to_owned(),
                taxonomy: TestTaxonomy::Gap,
            }
        );
    }

    #[test]
    fn taxonomy_coverage_rejects_raw_cargo_commands() {
        let mut records = records("dataflow");
        records[0].command = "cargo test";

        assert_eq!(
            validate_test_taxonomy_coverage(&records).expect_err("raw cargo should fail"),
            TestTaxonomyCoverageError::CommandDoesNotUseCargoFull {
                module: "dataflow".to_owned(),
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn taxonomy_coverage_accepts_committed_suite_artifacts() {
        let proof = validate_test_taxonomy_suite_artifacts(&suite_artifacts())
            .expect("Fix: committed taxonomy suite artifacts should pass");

        assert_eq!(proof.suite_count, 6);
        assert!(proof.total_file_count >= 2000);
    }

    #[test]
    fn taxonomy_coverage_rejects_suite_with_blockers() {
        let bad = r#"{
          "schema_version": 1,
          "suite": "unit",
          "file_count": 979,
          "vyre_file_count": 914,
          "dataflow_consumer_file_count": 56,
          "vyrec_file_count": 9,
          "files": [{"path": "/matching/vyre/test.rs", "has_test_entrypoint": true, "assertion_count": 1}],
          "blockers": ["broken"]
        }"#;

        assert_eq!(
            validate_test_taxonomy_suite_artifacts(&[("unit", bad, 1, 1, 0, 0)])
                .expect_err("blocked suite should fail"),
            TestTaxonomyCoverageError::SuiteArtifactMissingEvidence {
                suite: "unit",
                evidence: "zero blockers",
            }
        );
    }

    fn records(module: &'static str) -> Vec<TestCoverageRecord<'static>> {
        vec![
            record(module, TestTaxonomy::Unit),
            record(module, TestTaxonomy::Adversarial),
            record(module, TestTaxonomy::Property),
            record(module, TestTaxonomy::Benchmark),
            record(module, TestTaxonomy::Fuzz),
            record(module, TestTaxonomy::Gap),
        ]
    }

    fn record(module: &'static str, taxonomy: TestTaxonomy) -> TestCoverageRecord<'static> {
        TestCoverageRecord {
            module,
            taxonomy,
            command: "./cargo_full test -j1 -p vyre-self-substrate",
            evidence: "release/test-taxonomy.md",
        }
    }

    fn suite_artifacts() -> Vec<(&'static str, &'static str, u64, u64, u64, u64)> {
        vec![
            (
                "unit",
                include_str!("../../../../release/evidence/tests/unit-suite.json"),
                900,
                800,
                50,
                1,
            ),
            (
                "adversarial",
                include_str!("../../../../release/evidence/tests/adversarial-suite.json"),
                100,
                100,
                1,
                1,
            ),
            (
                "property",
                include_str!("../../../../release/evidence/tests/property-suite.json"),
                50,
                40,
                10,
                1,
            ),
            (
                "benchmark",
                include_str!("../../../../release/evidence/tests/benchmark-suite.json"),
                900,
                800,
                1,
                1,
            ),
            (
                "fuzz",
                include_str!("../../../../release/evidence/tests/fuzz-suite.json"),
                10,
                5,
                1,
                1,
            ),
            (
                "gap",
                include_str!("../../../../release/evidence/tests/gap-suite.json"),
                20,
                20,
                1,
                1,
            ),
        ]
    }
}

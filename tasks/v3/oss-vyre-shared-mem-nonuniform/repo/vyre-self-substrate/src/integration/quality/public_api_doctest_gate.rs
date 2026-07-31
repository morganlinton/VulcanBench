//! Public API doctest drift validation.

/// One public documentation example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiDoctestRecord<'a> {
    /// Documentation path or module.
    pub doc_path: &'a str,
    /// Public API symbol demonstrated by the example.
    pub public_symbol: &'a str,
    /// Exact cargo_full command that compiles the example.
    pub command: &'a str,
    /// Whether the example compiled against the current API.
    pub compiled: bool,
}

/// Public API doctest proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiDoctestProof {
    /// Number of examples validated.
    pub example_count: usize,
}

/// Committed public docs/API evidence proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicApiDocsArtifactProof {
    /// Vyre public example count.
    pub vyre_example_count: u64,
    /// Dataflow consumer public example count.
    pub dataflow_example_count: u64,
}

/// Public API doctest validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicApiDoctestError {
    /// No doctest records supplied.
    EmptyRecords,
    /// Metadata is empty.
    EmptyMetadata {
        /// Doc path.
        doc_path: String,
        /// Field.
        field: &'static str,
    },
    /// Doctest command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Doc path.
        doc_path: String,
        /// Command.
        command: String,
    },
    /// Example does not compile.
    ExampleDoesNotCompile {
        /// Doc path.
        doc_path: String,
        /// Public symbol.
        public_symbol: String,
    },
    /// Example exposes internal implementation detail.
    InternalApiInExample {
        /// Doc path.
        doc_path: String,
        /// Public symbol field.
        public_symbol: String,
    },
    /// Committed public API/docs artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed public API/docs artifact is missing a numeric field.
    ArtifactMissingNumber {
        /// Missing field.
        field: &'static str,
    },
    /// Committed public API/docs artifact missed a threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for PublicApiDoctestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "public API doctest records are empty. Fix: compile public examples against real APIs before release."
            ),
            Self::EmptyMetadata { doc_path, field } => write!(
                f,
                "public API doctest `{doc_path}` has empty {field}. Fix: record doc path, public symbol, command, and compile result."
            ),
            Self::CommandDoesNotUseCargoFull { doc_path, command } => write!(
                f,
                "public API doctest `{doc_path}` uses `{command}` instead of ./cargo_full. Fix: compile docs through cargo_full."
            ),
            Self::ExampleDoesNotCompile {
                doc_path,
                public_symbol,
            } => write!(
                f,
                "public API doctest `{doc_path}` for `{public_symbol}` does not compile. Fix: update the example or real API before release."
            ),
            Self::InternalApiInExample {
                doc_path,
                public_symbol,
            } => write!(
                f,
                "public API doctest `{doc_path}` demonstrates internal API `{public_symbol}`. Fix: examples must use stable public contracts only."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "public API docs artifact is missing {evidence}. Fix: connect public examples to committed README and docs evidence."
            ),
            Self::ArtifactMissingNumber { field } => write!(
                f,
                "public API docs artifact has no numeric {field}. Fix: record exact public example counters."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "public API docs artifact {field}={observed} missed required {required}. Fix: add real public examples before launch."
            ),
        }
    }
}

impl std::error::Error for PublicApiDoctestError {}

/// Validate public API doctest evidence.
pub fn validate_public_api_doctests(
    records: &[PublicApiDoctestRecord<'_>],
) -> Result<PublicApiDoctestProof, PublicApiDoctestError> {
    if records.is_empty() {
        return Err(PublicApiDoctestError::EmptyRecords);
    }
    for record in records {
        for (field, value) in [
            ("doc_path", record.doc_path),
            ("public_symbol", record.public_symbol),
            ("command", record.command),
        ] {
            if value.trim().is_empty() {
                return Err(PublicApiDoctestError::EmptyMetadata {
                    doc_path: record.doc_path.to_owned(),
                    field,
                });
            }
        }
        if !record.command.trim_start().starts_with("./cargo_full ") {
            return Err(PublicApiDoctestError::CommandDoesNotUseCargoFull {
                doc_path: record.doc_path.to_owned(),
                command: record.command.to_owned(),
            });
        }
        if !record.compiled {
            return Err(PublicApiDoctestError::ExampleDoesNotCompile {
                doc_path: record.doc_path.to_owned(),
                public_symbol: record.public_symbol.to_owned(),
            });
        }
        if is_internal_symbol(record.public_symbol) {
            return Err(PublicApiDoctestError::InternalApiInExample {
                doc_path: record.doc_path.to_owned(),
                public_symbol: record.public_symbol.to_owned(),
            });
        }
    }
    Ok(PublicApiDoctestProof {
        example_count: records.len(),
    })
}

fn is_internal_symbol(symbol: &str) -> bool {
    let lower = symbol.to_ascii_lowercase();
    lower.contains("internal")
        || lower.contains("private")
        || lower.contains("staging")
        || lower.contains("scratch")
        || lower.contains("pipeline::")
}

/// Validate committed public README/docs evidence.
pub fn validate_public_api_docs_artifacts(
    vyre_readme_contracts: &str,
    readme_contracts: &str,
    docs_matrix: &str,
    vyre_readme_proof: &str,
    readme_proof: &str,
) -> Result<PublicApiDocsArtifactProof, PublicApiDoctestError> {
    for (evidence, needle) in [
        ("Vyre README exists", "\"exists\": true"),
        ("Vyre no missing tokens", "\"missing_tokens\": []"),
        ("Vyre zero blockers", "\"blockers\": []"),
        ("Vyre version token", "\"0.6.3\""),
        ("Vyre crate token", "\"vyre\""),
        ("CUDA token", "\"cuda\""),
        ("WGPU token", "\"wgpu\""),
        ("public Program token", "\"vyre::program\""),
        ("cargo add vyre token", "\"cargo add vyre\""),
        ("release evidence token", "\"release/evidence\""),
    ] {
        artifact_contains(vyre_readme_contracts, evidence, needle)?;
    }
    for (evidence, needle) in [
        ("Dataflow consumer README exists", "\"exists\": true"),
        (
            "Dataflow consumer no missing tokens",
            "\"missing_tokens\": []",
        ),
        ("Dataflow consumer zero blockers", "\"blockers\": []"),
        ("Dataflow consumer version token", "\"0.1.0\""),
        ("dataflow token", "\"dataflow\""),
        ("SSA token", "\"ssa\""),
        ("IFDS token", "\"ifds\""),
        ("points-to token", "\"points-to\""),
        ("serde evidence token", "\"serde_evidence\""),
        (
            "cargo add dataflow consumer token",
            "\"cargo add dataflow-consumer\"",
        ),
    ] {
        artifact_contains(readme_contracts, evidence, needle)?;
    }
    for (evidence, needle) in [
        ("docs matrix schema", "\"schema_version\": 5"),
        ("docs matrix blockers inventory", "\"blockers\""),
        (
            "curated proof docs preserved",
            "\"curated_proof_docs_preserved\"",
        ),
        ("docs list", "\"docs\""),
    ] {
        artifact_contains(docs_matrix, evidence, needle)?;
    }
    for (evidence, needle) in [
        ("Vyre README proof title", "# Vyre README proof"),
        ("Vyre CUDA-first contract", "CUDA-first/WGPU-fallback"),
        (
            "Vyre evidence contract",
            "concrete release evidence artifacts",
        ),
        ("Vyre example-block contract", "at least one example block"),
    ] {
        artifact_contains(vyre_readme_proof, evidence, needle)?;
    }
    for (evidence, needle) in [
        (
            "Dataflow consumer README proof title",
            "# Dataflow consumer README proof",
        ),
        ("Dataflow consumer API surface contract", "0.1.0"),
        (
            "Dataflow consumer standalone API contract",
            "standalone Dataflow consumer APIs",
        ),
        (
            "Dataflow consumer serde evidence contract",
            "serde_evidence",
        ),
    ] {
        artifact_contains(readme_proof, evidence, needle)?;
    }

    let vyre_example_count = artifact_number_field(vyre_readme_contracts, "example_count")?;
    let dataflow_example_count = artifact_number_field(readme_contracts, "example_count")?;
    artifact_at_least("vyre example_count", vyre_example_count, 1)?;
    artifact_at_least("dataflow consumer example_count", dataflow_example_count, 1)?;

    Ok(PublicApiDocsArtifactProof {
        vyre_example_count,
        dataflow_example_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), PublicApiDoctestError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(PublicApiDoctestError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), PublicApiDoctestError> {
    if observed >= required {
        Ok(())
    } else {
        Err(PublicApiDoctestError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn artifact_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, PublicApiDoctestError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(PublicApiDoctestError::ArtifactMissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(PublicApiDoctestError::ArtifactMissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(PublicApiDoctestError::ArtifactMissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| PublicApiDoctestError::ArtifactMissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_doctests_accept_compiling_public_examples() {
        let proof = validate_public_api_doctests(&[
            record("README.md", "vyre::registered_backends", true),
            record("vyre-driver-cuda/README.md", "CudaBackend", true),
        ])
        .expect("Fix: compiling public API examples should pass");

        assert_eq!(proof.example_count, 2);
    }

    #[test]
    fn public_api_doctests_reject_raw_cargo_and_compile_failure() {
        let mut raw_cargo = record("README.md", "vyre::registered_backends", true);
        raw_cargo.command = "cargo test --doc";
        assert_eq!(
            validate_public_api_doctests(&[raw_cargo]).expect_err("raw cargo should fail"),
            PublicApiDoctestError::CommandDoesNotUseCargoFull {
                doc_path: "README.md".to_owned(),
                command: "cargo test --doc".to_owned(),
            }
        );

        assert_eq!(
            validate_public_api_doctests(&[record("README.md", "vyre::missing", false)])
                .expect_err("compile failure should fail"),
            PublicApiDoctestError::ExampleDoesNotCompile {
                doc_path: "README.md".to_owned(),
                public_symbol: "vyre::missing".to_owned(),
            }
        );
    }

    #[test]
    fn public_api_doctests_reject_internal_examples() {
        assert_eq!(
            validate_public_api_doctests(&[record(
                "README.md",
                "vyre_driver_cuda::pipeline::CompiledDispatch",
                true,
            )])
            .expect_err("internal API example should fail"),
            PublicApiDoctestError::InternalApiInExample {
                doc_path: "README.md".to_owned(),
                public_symbol: "vyre_driver_cuda::pipeline::CompiledDispatch".to_owned(),
            }
        );
    }

    #[test]
    fn public_api_doctests_accept_committed_readme_and_docs_artifacts() {
        let proof = validate_public_api_docs_artifacts(
            include_str!("../../../../release/evidence/docs/vyre-readme-contracts.json"),
            include_str!("../../../../release/evidence/dataflow/readme-contracts.json"),
            include_str!("../../../../release/evidence/docs/docs-matrix.json"),
            include_str!("../../../../release/evidence/docs/vyre-readme-proof.md"),
            include_str!("../../../../release/evidence/dataflow/readme-proof.md"),
        )
        .expect("Fix: committed public docs artifacts should pass");

        assert!(proof.vyre_example_count >= 1);
        assert!(proof.dataflow_example_count >= 1);
    }

    #[test]
    fn public_api_doctests_reject_missing_public_examples() {
        let vyre = r#"{
          "exists": true,
          "missing_tokens": [],
          "blockers": [],
          "required_tokens": ["0.6.3", "vyre", "cuda", "wgpu", "vyre::program", "cargo add vyre", "release/evidence"],
          "example_count": 0
        }"#;

        assert_eq!(
            validate_public_api_docs_artifacts(
                vyre,
                include_str!("../../../../release/evidence/dataflow/readme-contracts.json"),
                include_str!("../../../../release/evidence/docs/docs-matrix.json"),
                include_str!("../../../../release/evidence/docs/vyre-readme-proof.md"),
                include_str!("../../../../release/evidence/dataflow/readme-proof.md"),
            )
            .expect_err("zero public examples should fail"),
            PublicApiDoctestError::ArtifactThresholdMiss {
                field: "vyre example_count",
                observed: 0,
                required: 1,
            }
        );
    }

    fn record(
        doc_path: &'static str,
        public_symbol: &'static str,
        compiled: bool,
    ) -> PublicApiDoctestRecord<'static> {
        PublicApiDoctestRecord {
            doc_path,
            public_symbol,
            command: "./cargo_full test -j1 --doc",
            compiled,
        }
    }
}

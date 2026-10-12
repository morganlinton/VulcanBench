//! Vyrec GPU preprocessing coverage validation.

use std::collections::BTreeSet;

/// GPU preprocessing capability required before parser input.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GpuPreprocessingCapability {
    /// Macro expansion.
    MacroExpansion,
    /// Conditional inclusion.
    ConditionalInclusion,
    /// Include graph tracking.
    IncludeGraphTracking,
    /// Token provenance.
    TokenProvenance,
    /// Line marker tracking.
    LineMarkers,
    /// Macro stringification.
    Stringification,
    /// Token pasting.
    TokenPasting,
    /// Variadic macros.
    VariadicMacros,
    /// Builtin macros.
    BuiltinMacros,
}

/// GPU token class required by preprocessing/lexing.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GpuTokenClass {
    /// Comments.
    Comments,
    /// Identifiers.
    Identifiers,
    /// Literals.
    Literals,
    /// Punctuation.
    Punctuation,
    /// Whitespace.
    Whitespace,
    /// Directives.
    Directives,
    /// String and character states.
    StringCharStates,
}

/// One GPU preprocessing capability evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuPreprocessingCapabilityRecord<'a> {
    /// Covered capability.
    pub capability: GpuPreprocessingCapability,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path or test.
    pub evidence: &'a str,
}

/// One GPU token-class evidence record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuTokenClassRecord<'a> {
    /// Covered token class.
    pub class: GpuTokenClass,
    /// Exact cargo_full command.
    pub command: &'a str,
    /// Evidence path or test.
    pub evidence: &'a str,
}

/// GPU preprocessing coverage proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuPreprocessingCoverageProof {
    /// Capability count.
    pub capability_count: usize,
    /// Token-class count.
    pub token_class_count: usize,
}

/// Committed Linux GPU preprocessing artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuPreprocessingLinuxArtifactProof {
    /// Linux C files covered by the artifact.
    pub total_files: u64,
    /// Linux C source bytes covered by the artifact.
    pub total_source_bytes: u64,
    /// Preprocessor pipeline cache hits.
    pub preprocessor_pipeline_cache_hits: u64,
    /// Include cache bytes stored.
    pub include_cache_bytes_stored: u64,
}

/// GPU preprocessing coverage errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuPreprocessingCoverageError {
    /// No capability evidence supplied.
    EmptyCapabilities,
    /// No token-class evidence supplied.
    EmptyTokenClasses,
    /// Capability metadata is empty.
    EmptyCapabilityMetadata {
        /// Capability.
        capability: GpuPreprocessingCapability,
        /// Field.
        field: &'static str,
    },
    /// Token-class metadata is empty.
    EmptyTokenClassMetadata {
        /// Token class.
        class: GpuTokenClass,
        /// Field.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Command.
        command: String,
    },
    /// Required capability is missing.
    MissingCapability {
        /// Missing capability.
        capability: GpuPreprocessingCapability,
    },
    /// Required token class is missing.
    MissingTokenClass {
        /// Missing token class.
        class: GpuTokenClass,
    },
}

impl std::fmt::Display for GpuPreprocessingCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCapabilities => write!(
                f,
                "GPU preprocessing capability coverage is empty. Fix: add evidence for macro expansion, includes, provenance, line markers, stringification, token pasting, variadics, and builtins."
            ),
            Self::EmptyTokenClasses => write!(
                f,
                "GPU token-class coverage is empty. Fix: add evidence for comments, identifiers, literals, punctuation, whitespace, directives, and string/char states."
            ),
            Self::EmptyCapabilityMetadata { capability, field } => write!(
                f,
                "GPU preprocessing capability {capability:?} has empty {field}. Fix: every record needs command and evidence."
            ),
            Self::EmptyTokenClassMetadata { class, field } => write!(
                f,
                "GPU token class {class:?} has empty {field}. Fix: every record needs command and evidence."
            ),
            Self::CommandDoesNotUseCargoFull { command } => write!(
                f,
                "GPU preprocessing coverage command `{command}` does not use ./cargo_full. Fix: run preprocessing evidence through cargo_full."
            ),
            Self::MissingCapability { capability } => write!(
                f,
                "GPU preprocessing coverage is missing {capability:?}. Fix: add explicit parity evidence for that preprocessing capability."
            ),
            Self::MissingTokenClass { class } => write!(
                f,
                "GPU token-class coverage is missing {class:?}. Fix: add explicit token classification evidence for that class."
            ),
        }
    }
}

impl std::error::Error for GpuPreprocessingCoverageError {}

/// Committed Linux GPU preprocessing artifact validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuPreprocessingLinuxArtifactError {
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

impl std::fmt::Display for GpuPreprocessingLinuxArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "GPU preprocessing Linux artifact is missing {field}. Fix: commit CUDA preprocessing evidence over the Linux C corpus."
            ),
            Self::MissingNumber { field } => write!(
                f,
                "GPU preprocessing Linux artifact has no numeric {field}. Fix: record the exact release preprocessing counter."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "GPU preprocessing Linux artifact {field}={observed} missed required {required}. Fix: keep preprocessing on the CUDA path and remove host-token staging."
            ),
        }
    }
}

impl std::error::Error for GpuPreprocessingLinuxArtifactError {}

const REQUIRED_CAPABILITIES: &[GpuPreprocessingCapability] = &[
    GpuPreprocessingCapability::MacroExpansion,
    GpuPreprocessingCapability::ConditionalInclusion,
    GpuPreprocessingCapability::IncludeGraphTracking,
    GpuPreprocessingCapability::TokenProvenance,
    GpuPreprocessingCapability::LineMarkers,
    GpuPreprocessingCapability::Stringification,
    GpuPreprocessingCapability::TokenPasting,
    GpuPreprocessingCapability::VariadicMacros,
    GpuPreprocessingCapability::BuiltinMacros,
];

const REQUIRED_TOKEN_CLASSES: &[GpuTokenClass] = &[
    GpuTokenClass::Comments,
    GpuTokenClass::Identifiers,
    GpuTokenClass::Literals,
    GpuTokenClass::Punctuation,
    GpuTokenClass::Whitespace,
    GpuTokenClass::Directives,
    GpuTokenClass::StringCharStates,
];

/// Validate GPU preprocessing and token-class coverage.
pub fn validate_gpu_preprocessing_coverage(
    capabilities: &[GpuPreprocessingCapabilityRecord<'_>],
    token_classes: &[GpuTokenClassRecord<'_>],
) -> Result<GpuPreprocessingCoverageProof, GpuPreprocessingCoverageError> {
    if capabilities.is_empty() {
        return Err(GpuPreprocessingCoverageError::EmptyCapabilities);
    }
    if token_classes.is_empty() {
        return Err(GpuPreprocessingCoverageError::EmptyTokenClasses);
    }

    let mut covered_capabilities = BTreeSet::new();
    for record in capabilities {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(GpuPreprocessingCoverageError::EmptyCapabilityMetadata {
                    capability: record.capability,
                    field,
                });
            }
        }
        require_cargo_full(record.command)?;
        covered_capabilities.insert(record.capability);
    }

    let mut covered_token_classes = BTreeSet::new();
    for record in token_classes {
        for (field, value) in [("command", record.command), ("evidence", record.evidence)] {
            if value.trim().is_empty() {
                return Err(GpuPreprocessingCoverageError::EmptyTokenClassMetadata {
                    class: record.class,
                    field,
                });
            }
        }
        require_cargo_full(record.command)?;
        covered_token_classes.insert(record.class);
    }

    for capability in REQUIRED_CAPABILITIES {
        if !covered_capabilities.contains(capability) {
            return Err(GpuPreprocessingCoverageError::MissingCapability {
                capability: *capability,
            });
        }
    }
    for class in REQUIRED_TOKEN_CLASSES {
        if !covered_token_classes.contains(class) {
            return Err(GpuPreprocessingCoverageError::MissingTokenClass { class: *class });
        }
    }

    Ok(GpuPreprocessingCoverageProof {
        capability_count: covered_capabilities.len(),
        token_class_count: covered_token_classes.len(),
    })
}

fn require_cargo_full(command: &str) -> Result<(), GpuPreprocessingCoverageError> {
    if command.trim_start().starts_with("./cargo_full ") {
        Ok(())
    } else {
        Err(GpuPreprocessingCoverageError::CommandDoesNotUseCargoFull {
            command: command.to_owned(),
        })
    }
}

/// Validate the committed Linux CUDA preprocessing artifact.
pub fn validate_gpu_preprocessing_linux_artifact(
    artifact: &str,
) -> Result<GpuPreprocessingLinuxArtifactProof, GpuPreprocessingLinuxArtifactError> {
    preproc_contains(
        artifact,
        "raw GPU lexer input",
        "\"compile_tu_lexer_input_mode\": \"raw_bytes_gpu_lex\"",
    )?;
    preproc_contains(
        artifact,
        "raw GPU preprocessor input",
        "\"compile_tu_preprocessor_input_mode\": \"raw_bytes_gpu_preprocess\"",
    )?;
    preproc_contains(
        artifact,
        "CUDA parser backend",
        "\"resident_vyre_parse_backend_id\": \"cuda\"",
    )?;
    preproc_contains(
        artifact,
        "raw GPU syntax input",
        "\"resident_vyre_parse_input_mode\": \"raw_bytes_gpu_syntax\"",
    )?;
    preproc_contains(artifact, "Linux macro state", "\"__KERNEL__=1\"")?;
    preproc_contains(artifact, "Linux x86 macro state", "\"CONFIG_X86_64=1\"")?;
    preproc_contains(artifact, "Linux include dirs", "include/uapi")?;

    let total_files = preproc_number_field(artifact, "total_files")?;
    let total_source_bytes = preproc_number_field(artifact, "total_source_bytes")?;
    let preprocessor_pipeline_cache_hits =
        preproc_number_field(artifact, "preprocessor_pipeline_cache_hits")?;
    let preprocessor_pipeline_cache_misses =
        preproc_number_field(artifact, "preprocessor_pipeline_cache_misses")?;
    let preprocessor_pipeline_cache_evictions =
        preproc_number_field(artifact, "preprocessor_pipeline_cache_evictions")?;
    let macro_state_cache_hits = preproc_number_field(artifact, "macro_state_cache_hits")?;
    let macro_state_cache_misses = preproc_number_field(artifact, "macro_state_cache_misses")?;
    let include_cache_hits = preproc_number_field(artifact, "include_cache_hits")?;
    let include_cache_misses = preproc_number_field(artifact, "include_cache_misses")?;
    let include_cache_bytes_stored = preproc_number_field(artifact, "include_cache_bytes_stored")?;
    let host_token_upload = preproc_number_field(
        artifact,
        "resident_vyre_parse_host_token_stream_upload_bytes",
    )?;

    preproc_at_least("total_files", total_files, 250)?;
    preproc_at_least("total_source_bytes", total_source_bytes, 4 * 1024 * 1024)?;
    preproc_at_least(
        "preprocessor_pipeline_cache_hits",
        preprocessor_pipeline_cache_hits,
        1,
    )?;
    preproc_at_least(
        "preprocessor_pipeline_cache_misses",
        preprocessor_pipeline_cache_misses,
        1,
    )?;
    preproc_exact(
        "preprocessor_pipeline_cache_evictions",
        preprocessor_pipeline_cache_evictions,
        0,
    )?;
    preproc_at_least("macro_state_cache_hits", macro_state_cache_hits, 1)?;
    preproc_at_least("macro_state_cache_misses", macro_state_cache_misses, 1)?;
    preproc_at_least("include_cache_hits", include_cache_hits, 1)?;
    preproc_at_least("include_cache_misses", include_cache_misses, 1)?;
    preproc_at_least(
        "include_cache_bytes_stored",
        include_cache_bytes_stored,
        total_source_bytes,
    )?;
    preproc_exact(
        "resident_vyre_parse_host_token_stream_upload_bytes",
        host_token_upload,
        0,
    )?;

    Ok(GpuPreprocessingLinuxArtifactProof {
        total_files,
        total_source_bytes,
        preprocessor_pipeline_cache_hits,
        include_cache_bytes_stored,
    })
}

fn preproc_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), GpuPreprocessingLinuxArtifactError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(GpuPreprocessingLinuxArtifactError::MissingField { field })
    }
}

fn preproc_exact(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), GpuPreprocessingLinuxArtifactError> {
    if observed == required {
        Ok(())
    } else {
        Err(GpuPreprocessingLinuxArtifactError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn preproc_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), GpuPreprocessingLinuxArtifactError> {
    if observed >= required {
        Ok(())
    } else {
        Err(GpuPreprocessingLinuxArtifactError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn preproc_number_field(
    artifact: &str,
    field: &'static str,
) -> Result<u64, GpuPreprocessingLinuxArtifactError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(GpuPreprocessingLinuxArtifactError::MissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(GpuPreprocessingLinuxArtifactError::MissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(GpuPreprocessingLinuxArtifactError::MissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| GpuPreprocessingLinuxArtifactError::MissingNumber { field })
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn gpu_preprocessing_coverage_accepts_all_required_records() {
        let proof = validate_gpu_preprocessing_coverage(&capabilities(), &token_classes())
            .expect("Fix: complete GPU preprocessing coverage should pass");

        assert_eq!(proof.capability_count, 9);
        assert_eq!(proof.token_class_count, 7);
    }

    #[test]
    fn gpu_preprocessing_coverage_rejects_missing_builtin_macros() {
        let mut capabilities = capabilities();
        capabilities.pop();

        assert_eq!(
            validate_gpu_preprocessing_coverage(&capabilities, &token_classes())
                .expect_err("missing builtin macros should fail"),
            GpuPreprocessingCoverageError::MissingCapability {
                capability: GpuPreprocessingCapability::BuiltinMacros,
            }
        );
    }

    #[test]
    fn gpu_preprocessing_coverage_rejects_missing_string_char_states_and_raw_cargo() {
        let mut missing_token_classes = token_classes();
        missing_token_classes.pop();
        assert_eq!(
            validate_gpu_preprocessing_coverage(&capabilities(), &missing_token_classes)
                .expect_err("missing string/char states should fail"),
            GpuPreprocessingCoverageError::MissingTokenClass {
                class: GpuTokenClass::StringCharStates,
            }
        );

        let mut capabilities = capabilities();
        capabilities[0].command = "cargo test";
        assert_eq!(
            validate_gpu_preprocessing_coverage(&capabilities, &token_classes())
                .expect_err("raw cargo should fail"),
            GpuPreprocessingCoverageError::CommandDoesNotUseCargoFull {
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn gpu_preprocessing_linux_artifact_accepts_committed_cuda_linux_evidence() {
        let proof = validate_gpu_preprocessing_linux_artifact(include_str!(
            "../../../release/evidence/parser/c-parser-linux-subsystem.json"
        ))
        .expect("Fix: committed Linux CUDA preprocessing artifact should pass");

        assert!(proof.total_files >= 250);
        assert!(proof.total_source_bytes >= 4 * 1024 * 1024);
        assert!(proof.preprocessor_pipeline_cache_hits >= 1);
        assert!(proof.include_cache_bytes_stored >= proof.total_source_bytes);
    }

    #[test]
    fn gpu_preprocessing_linux_artifact_rejects_cpu_preprocessing() {
        let artifact = r#"{
          "compile_tu_lexer_input_mode": "raw_bytes_cpu_lex",
          "compile_tu_preprocessor_input_mode": "raw_bytes_gpu_preprocess",
          "resident_vyre_parse_backend_id": "cuda",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "macros": ["__KERNEL__=1", "CONFIG_X86_64=1"],
          "include_dirs": ["/linux/include/uapi"],
          "total_files": 490,
          "total_source_bytes": 7394810,
          "preprocessor_pipeline_cache_hits": 489,
          "preprocessor_pipeline_cache_misses": 1,
          "preprocessor_pipeline_cache_evictions": 0,
          "macro_state_cache_hits": 489,
          "macro_state_cache_misses": 1,
          "include_cache_hits": 489,
          "include_cache_misses": 1,
          "include_cache_bytes_stored": 7394810,
          "resident_vyre_parse_host_token_stream_upload_bytes": 0
        }"#;

        assert_eq!(
            validate_gpu_preprocessing_linux_artifact(artifact)
                .expect_err("CPU lexing should fail CUDA preprocessing release evidence"),
            GpuPreprocessingLinuxArtifactError::MissingField {
                field: "raw GPU lexer input",
            }
        );
    }

    #[test]
    fn gpu_preprocessing_linux_artifact_rejects_host_token_uploads() {
        let artifact = r#"{
          "compile_tu_lexer_input_mode": "raw_bytes_gpu_lex",
          "compile_tu_preprocessor_input_mode": "raw_bytes_gpu_preprocess",
          "resident_vyre_parse_backend_id": "cuda",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "macros": ["__KERNEL__=1", "CONFIG_X86_64=1"],
          "include_dirs": ["/linux/include/uapi"],
          "total_files": 490,
          "total_source_bytes": 7394810,
          "preprocessor_pipeline_cache_hits": 489,
          "preprocessor_pipeline_cache_misses": 1,
          "preprocessor_pipeline_cache_evictions": 0,
          "macro_state_cache_hits": 489,
          "macro_state_cache_misses": 1,
          "include_cache_hits": 489,
          "include_cache_misses": 1,
          "include_cache_bytes_stored": 7394810,
          "resident_vyre_parse_host_token_stream_upload_bytes": 64
        }"#;

        assert_eq!(
            validate_gpu_preprocessing_linux_artifact(artifact)
                .expect_err("host token upload should fail CUDA preprocessing release evidence"),
            GpuPreprocessingLinuxArtifactError::ThresholdMiss {
                field: "resident_vyre_parse_host_token_stream_upload_bytes",
                observed: 64,
                required: 0,
            }
        );
    }

    fn capabilities() -> Vec<GpuPreprocessingCapabilityRecord<'static>> {
        REQUIRED_CAPABILITIES
            .iter()
            .copied()
            .map(capability)
            .collect()
    }

    fn token_classes() -> Vec<GpuTokenClassRecord<'static>> {
        REQUIRED_TOKEN_CLASSES
            .iter()
            .copied()
            .map(token_class)
            .collect()
    }

    fn capability(
        capability: GpuPreprocessingCapability,
    ) -> GpuPreprocessingCapabilityRecord<'static> {
        GpuPreprocessingCapabilityRecord {
            capability,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-gpu-preprocessing.md",
        }
    }

    fn token_class(class: GpuTokenClass) -> GpuTokenClassRecord<'static> {
        GpuTokenClassRecord {
            class,
            command: "./cargo_full test -j1 -p vyrec",
            evidence: "release/parity/vyrec-gpu-token-classification.md",
        }
    }
}

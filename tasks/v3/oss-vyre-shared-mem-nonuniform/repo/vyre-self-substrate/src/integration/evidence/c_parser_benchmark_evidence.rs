//! C parser benchmark artifact validation.

/// Validated C parser benchmark artifact summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CParserBenchmarkEvidenceProof {
    /// Number of Linux subsystem C files covered.
    pub total_files: u64,
    /// Total C source bytes covered by the artifact.
    pub total_source_bytes: u64,
    /// Reported Vyre-vs-baseline speedup scaled by 1000.
    pub speedup_x1000: u64,
}

/// C parser benchmark evidence validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CParserBenchmarkEvidenceError {
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
        /// Required minimum.
        required: u64,
    },
}

impl std::fmt::Display for CParserBenchmarkEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "C parser benchmark evidence is missing {field}. Fix: commit Linux CUDA parser benchmark evidence with resident GPU parse details."
            ),
            Self::MissingNumber { field } => write!(
                f,
                "C parser benchmark evidence has no numeric {field}. Fix: record the exact release benchmark counter."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "C parser benchmark evidence {field}={observed} missed required {required}. Fix: improve the CUDA parser path or update the approved release target."
            ),
        }
    }
}

impl std::error::Error for CParserBenchmarkEvidenceError {}

/// Validate the committed Linux C parser throughput artifact.
pub fn validate_c_parser_benchmark_evidence(
    artifact: &str,
) -> Result<CParserBenchmarkEvidenceProof, CParserBenchmarkEvidenceError> {
    require_contains(
        artifact,
        "linux subsystem marker",
        "\"linux_subsystem_candidate\": true",
    )?;
    require_contains(
        artifact,
        "CUDA parser backend",
        "\"resident_vyre_parse_backend_id\": \"cuda\"",
    )?;
    require_contains(
        artifact,
        "raw GPU syntax input",
        "\"resident_vyre_parse_input_mode\": \"raw_bytes_gpu_syntax\"",
    )?;
    require_contains(
        artifact,
        "pipeline cache enabled",
        "\"resident_vyre_pipeline_cache_enabled\": true",
    )?;
    require_contains(
        artifact,
        "tree-sitter error-free baseline",
        "\"resident_tree_sitter_has_error\": false",
    )?;

    let total_files = number_field(artifact, "total_files")?;
    let total_source_bytes = number_field(artifact, "total_source_bytes")?;
    let samples = number_field(artifact, "resident_parse_bench_samples")?;
    let host_token_upload = number_field(
        artifact,
        "resident_vyre_parse_host_token_stream_upload_bytes",
    )?;
    let dispatch_count = number_field(artifact, "resident_vyre_parse_gpu_dispatch_count")?;
    let host_submit_count = number_field(artifact, "resident_vyre_parse_host_submit_count")?;
    let covered_tokens = number_field(artifact, "resident_vyre_parse_ast_covered_tokens")?;
    let token_count = number_field(artifact, "resident_vyre_parse_token_count")?;
    let speedup_x1000 = number_field(artifact, "resident_vyre_vs_tree_sitter_speedup_x1000")?;

    require_at_least("total_files", total_files, 250)?;
    require_at_least("total_source_bytes", total_source_bytes, 4 * 1024 * 1024)?;
    require_at_least("resident_parse_bench_samples", samples, 5)?;
    require_at_least("resident_vyre_parse_gpu_dispatch_count", dispatch_count, 1)?;
    require_at_least(
        "resident_vyre_parse_host_submit_count",
        host_submit_count,
        1,
    )?;
    if host_token_upload != 0 {
        return Err(CParserBenchmarkEvidenceError::ThresholdMiss {
            field: "resident_vyre_parse_host_token_stream_upload_bytes",
            observed: host_token_upload,
            required: 0,
        });
    }
    require_at_least(
        "resident_vyre_vs_tree_sitter_speedup_x1000",
        speedup_x1000,
        100_000,
    )?;
    if covered_tokens != token_count {
        return Err(CParserBenchmarkEvidenceError::ThresholdMiss {
            field: "resident_vyre_parse_ast_covered_tokens",
            observed: covered_tokens,
            required: token_count,
        });
    }

    Ok(CParserBenchmarkEvidenceProof {
        total_files,
        total_source_bytes,
        speedup_x1000,
    })
}

fn require_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), CParserBenchmarkEvidenceError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(CParserBenchmarkEvidenceError::MissingField { field })
    }
}

fn require_at_least(
    field: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), CParserBenchmarkEvidenceError> {
    if observed >= required {
        Ok(())
    } else {
        Err(CParserBenchmarkEvidenceError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn number_field(artifact: &str, field: &'static str) -> Result<u64, CParserBenchmarkEvidenceError> {
    let key = format!("\"{field}\"");
    let start = artifact
        .find(&key)
        .ok_or(CParserBenchmarkEvidenceError::MissingNumber { field })?;
    let after_key = &artifact[start + key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(CParserBenchmarkEvidenceError::MissingNumber { field })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(CParserBenchmarkEvidenceError::MissingNumber { field });
    }
    digits
        .parse::<u64>()
        .map_err(|_| CParserBenchmarkEvidenceError::MissingNumber { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_parser_benchmark_accepts_committed_linux_cuda_artifact() {
        let proof = validate_c_parser_benchmark_evidence(include_str!(
            "../../../../release/evidence/parser/c-parser-throughput.json"
        ))
        .expect("Fix: committed Linux CUDA C parser throughput evidence should pass");

        assert!(proof.total_files >= 250);
        assert!(proof.total_source_bytes >= 4 * 1024 * 1024);
        assert!(proof.speedup_x1000 >= 100_000);
    }

    #[test]
    fn c_parser_benchmark_rejects_host_token_uploads_and_low_speedup() {
        let artifact = r#"{
          "linux_subsystem_candidate": true,
          "resident_vyre_parse_backend_id": "cuda",
          "resident_vyre_parse_input_mode": "raw_bytes_gpu_syntax",
          "resident_vyre_pipeline_cache_enabled": true,
          "resident_tree_sitter_has_error": false,
          "total_files": 490,
          "total_source_bytes": 7394810,
          "resident_parse_bench_samples": 5,
          "resident_vyre_parse_host_token_stream_upload_bytes": 1,
          "resident_vyre_parse_gpu_dispatch_count": 8,
          "resident_vyre_parse_host_submit_count": 1,
          "resident_vyre_parse_ast_covered_tokens": 10,
          "resident_vyre_parse_token_count": 10,
          "resident_vyre_vs_tree_sitter_speedup_x1000": 1000
        }"#;

        assert_eq!(
            validate_c_parser_benchmark_evidence(artifact)
                .expect_err("host token upload should fail before speedup is trusted"),
            CParserBenchmarkEvidenceError::ThresholdMiss {
                field: "resident_vyre_parse_host_token_stream_upload_bytes",
                observed: 1,
                required: 0,
            }
        );
    }
}

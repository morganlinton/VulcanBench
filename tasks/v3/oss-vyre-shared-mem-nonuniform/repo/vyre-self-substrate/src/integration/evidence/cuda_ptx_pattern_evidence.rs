//! CUDA PTX pattern release evidence validation.

/// Validated CUDA PTX pattern artifact summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaPtxPatternEvidenceProof {
    /// PTX corpus kernels represented in the artifact.
    pub ptx_corpus_kernels: u64,
    /// PTX bytes emitted by the release corpus.
    pub ptx_bytes_emitted: u64,
    /// Vectorized load instructions emitted.
    pub vectorized_loads_emitted: u64,
    /// Vectorized store instructions emitted.
    pub vectorized_stores_emitted: u64,
}

/// CUDA PTX pattern evidence validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaPtxPatternEvidenceError {
    /// Required literal field is missing.
    MissingField {
        /// Missing field.
        field: &'static str,
    },
    /// Required metric is missing or malformed.
    MissingMetric {
        /// Missing metric.
        metric: &'static str,
    },
    /// Metric does not meet the release threshold.
    ThresholdMiss {
        /// Metric name.
        metric: &'static str,
        /// Observed value.
        observed: u64,
        /// Required value.
        required: u64,
    },
}

impl std::fmt::Display for CudaPtxPatternEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "CUDA PTX pattern artifact is missing {field}. Fix: commit release PTX evidence from the CUDA backend."
            ),
            Self::MissingMetric { metric } => write!(
                f,
                "CUDA PTX pattern artifact is missing metric {metric}. Fix: record PTX codegen counters for vectorization, predication, tensor cores, async copy, and source cache."
            ),
            Self::ThresholdMiss {
                metric,
                observed,
                required,
            } => write!(
                f,
                "CUDA PTX pattern artifact {metric}={observed} missed required {required}. Fix: restore the CUDA codegen optimization or update the approved release target."
            ),
        }
    }
}

impl std::error::Error for CudaPtxPatternEvidenceError {}

/// Validate the committed CUDA PTX pattern release artifact.
pub fn validate_cuda_ptx_pattern_evidence(
    artifact: &str,
) -> Result<CudaPtxPatternEvidenceProof, CudaPtxPatternEvidenceError> {
    require_contains(
        artifact,
        "selected CUDA backend",
        "\"selected_backend\": \"cuda\"",
    )?;
    require_contains(artifact, "GPU environment", "\"has_gpu\": true")?;
    require_contains(
        artifact,
        "RTX CUDA hardware",
        "\"name\": \"NVIDIA GeForce RTX 5090\"",
    )?;
    require_contains(
        artifact,
        "CUDA runtime version",
        "\"nvidia_cuda_version\": \"12.8\"",
    )?;
    require_contains(
        artifact,
        "PTX pattern case",
        "\"cuda.ptx.patterns.release.corpus\"",
    )?;
    require_contains(
        artifact,
        "PTX backend owner",
        "\"owner_crate\": \"vyre-emit-ptx\"",
    )?;
    require_contains(artifact, "CUDA case backend", "\"backend_id\": \"cuda\"")?;
    require_contains(artifact, "passing case", "\"status\": \"pass\"")?;
    require_contains(artifact, "exact correctness", "\"correctness\": \"Exact\"")?;

    let ptx_corpus_kernels = metric_p50(artifact, "ptx_corpus_kernels")?;
    let ptx_bytes_emitted = metric_p50(artifact, "ptx_bytes_emitted")?;
    let vectorized_loads_emitted = metric_p50(artifact, "ptx_vectorized_loads_emitted")?;
    let vectorized_stores_emitted = metric_p50(artifact, "ptx_vectorized_stores_emitted")?;
    let predicated_stores = metric_p50(artifact, "ptx_predicated_stores")?;
    let predication_candidates = metric_p50(artifact, "ptx_predication_candidates")?;
    let safe_predication_candidates = metric_p50(artifact, "ptx_safe_predication_candidates")?;
    let scheduled_fillers = metric_p50(artifact, "ptx_scheduled_fillers")?;
    let tensor_core_candidates = metric_p50(artifact, "ptx_tensor_core_candidates")?;
    let mma_sync_emitted = metric_p50(artifact, "ptx_mma_sync_emitted")?;
    let ldmatrix_capable_targets = metric_p50(artifact, "ptx_ldmatrix_capable_targets")?;
    let async_copy_candidates = metric_p50(artifact, "ptx_async_copy_candidates")?;
    let cp_async_emitted = metric_p50(artifact, "ptx_cp_async_emitted")?;
    let vec_load_candidates = metric_p50(artifact, "ptx_vec_load_candidates")?;
    let vec_store_candidates = metric_p50(artifact, "ptx_vec_store_candidates")?;
    let scalar_loads = metric_p50(artifact, "ptx_vector_kernel_scalar_loads")?;
    let scalar_stores = metric_p50(artifact, "ptx_vector_kernel_scalar_stores")?;
    let scalar_index_adds = metric_p50(artifact, "ptx_vector_kernel_scalar_index_adds")?;
    let cache_entries = metric_p50(artifact, "cuda_ptx_source_cache_entries")?;
    let cache_hits = metric_p50(artifact, "cuda_ptx_source_cache_hits")?;
    let cache_misses = metric_p50(artifact, "cuda_ptx_source_cache_misses")?;

    require_at_least("ptx_corpus_kernels", ptx_corpus_kernels, 4)?;
    require_at_least("ptx_bytes_emitted", ptx_bytes_emitted, 1024)?;
    require_at_least("ptx_vec_load_candidates", vec_load_candidates, 1)?;
    require_at_least("ptx_vec_store_candidates", vec_store_candidates, 1)?;
    require_at_least("ptx_vectorized_loads_emitted", vectorized_loads_emitted, 1)?;
    require_at_least(
        "ptx_vectorized_stores_emitted",
        vectorized_stores_emitted,
        1,
    )?;
    require_at_least("ptx_predication_candidates", predication_candidates, 1)?;
    require_at_least(
        "ptx_safe_predication_candidates",
        safe_predication_candidates,
        1,
    )?;
    require_at_least("ptx_predicated_stores", predicated_stores, 1)?;
    require_at_least("ptx_scheduled_fillers", scheduled_fillers, 1)?;
    require_at_least("ptx_tensor_core_candidates", tensor_core_candidates, 1)?;
    require_at_least("ptx_mma_sync_emitted", mma_sync_emitted, 1)?;
    require_at_least("ptx_ldmatrix_capable_targets", ldmatrix_capable_targets, 1)?;
    require_at_least("ptx_async_copy_candidates", async_copy_candidates, 1)?;
    require_at_least("ptx_cp_async_emitted", cp_async_emitted, 1)?;
    require_at_least("cuda_ptx_source_cache_entries", cache_entries, 1)?;
    require_at_least("cuda_ptx_source_cache_hits", cache_hits, 1)?;
    require_at_least("cuda_ptx_source_cache_misses", cache_misses, 1)?;
    require_exact("ptx_vector_kernel_scalar_loads", scalar_loads, 0)?;
    require_exact("ptx_vector_kernel_scalar_stores", scalar_stores, 0)?;
    require_exact("ptx_vector_kernel_scalar_index_adds", scalar_index_adds, 0)?;

    Ok(CudaPtxPatternEvidenceProof {
        ptx_corpus_kernels,
        ptx_bytes_emitted,
        vectorized_loads_emitted,
        vectorized_stores_emitted,
    })
}

fn require_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), CudaPtxPatternEvidenceError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(CudaPtxPatternEvidenceError::MissingField { field })
    }
}

fn require_at_least(
    metric: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), CudaPtxPatternEvidenceError> {
    if observed >= required {
        Ok(())
    } else {
        Err(CudaPtxPatternEvidenceError::ThresholdMiss {
            metric,
            observed,
            required,
        })
    }
}

fn require_exact(
    metric: &'static str,
    observed: u64,
    required: u64,
) -> Result<(), CudaPtxPatternEvidenceError> {
    if observed == required {
        Ok(())
    } else {
        Err(CudaPtxPatternEvidenceError::ThresholdMiss {
            metric,
            observed,
            required,
        })
    }
}

fn metric_p50(artifact: &str, metric: &'static str) -> Result<u64, CudaPtxPatternEvidenceError> {
    let key = format!("\"{metric}\"");
    let start = artifact
        .find(&key)
        .ok_or(CudaPtxPatternEvidenceError::MissingMetric { metric })?;
    let metric_body = &artifact[start + key.len()..];
    let p50_key = "\"p50\"";
    let p50_start = metric_body
        .find(p50_key)
        .ok_or(CudaPtxPatternEvidenceError::MissingMetric { metric })?;
    let after_key = &metric_body[p50_start + p50_key.len()..];
    let colon = after_key
        .find(':')
        .ok_or(CudaPtxPatternEvidenceError::MissingMetric { metric })?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return Err(CudaPtxPatternEvidenceError::MissingMetric { metric });
    }
    digits
        .parse::<u64>()
        .map_err(|_| CudaPtxPatternEvidenceError::MissingMetric { metric })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_ptx_pattern_evidence_accepts_committed_release_artifact() {
        let proof = validate_cuda_ptx_pattern_evidence(include_str!(
            "../../../../release/evidence/benchmarks/cuda-ptx-patterns.json"
        ))
        .expect("Fix: committed CUDA PTX pattern evidence should pass");

        assert!(proof.ptx_corpus_kernels >= 4);
        assert!(proof.ptx_bytes_emitted >= 1024);
        assert!(proof.vectorized_loads_emitted >= 1);
        assert!(proof.vectorized_stores_emitted >= 1);
    }

    #[test]
    fn cuda_ptx_pattern_evidence_rejects_wgpu_or_cpu_artifact() {
        let artifact = r#"{
          "selected_backend": "wgpu",
          "has_gpu": true,
          "name": "NVIDIA GeForce RTX 5090",
          "nvidia_cuda_version": "12.8",
          "id": "cuda.ptx.patterns.release.corpus",
          "owner_crate": "vyre-emit-ptx",
          "backend_id": "cuda",
          "status": "pass",
          "correctness": "Exact"
        }"#;

        assert_eq!(
            validate_cuda_ptx_pattern_evidence(artifact)
                .expect_err("non-CUDA selected backend should fail"),
            CudaPtxPatternEvidenceError::MissingField {
                field: "selected CUDA backend",
            }
        );
    }

    #[test]
    fn cuda_ptx_pattern_evidence_rejects_scalarized_vector_kernel() {
        let artifact = r#"{
          "selected_backend": "cuda",
          "has_gpu": true,
          "name": "NVIDIA GeForce RTX 5090",
          "nvidia_cuda_version": "12.8",
          "id": "cuda.ptx.patterns.release.corpus",
          "owner_crate": "vyre-emit-ptx",
          "backend_id": "cuda",
          "status": "pass",
          "correctness": "Exact",
          "ptx_corpus_kernels": {"p50": 8},
          "ptx_bytes_emitted": {"p50": 10785},
          "ptx_vectorized_loads_emitted": {"p50": 1},
          "ptx_vectorized_stores_emitted": {"p50": 1},
          "ptx_predicated_stores": {"p50": 14},
          "ptx_predication_candidates": {"p50": 2},
          "ptx_safe_predication_candidates": {"p50": 2},
          "ptx_scheduled_fillers": {"p50": 2},
          "ptx_tensor_core_candidates": {"p50": 3},
          "ptx_mma_sync_emitted": {"p50": 1},
          "ptx_ldmatrix_capable_targets": {"p50": 8},
          "ptx_async_copy_candidates": {"p50": 1},
          "ptx_cp_async_emitted": {"p50": 1},
          "ptx_vec_load_candidates": {"p50": 1},
          "ptx_vec_store_candidates": {"p50": 1},
          "ptx_vector_kernel_scalar_loads": {"p50": 1},
          "ptx_vector_kernel_scalar_stores": {"p50": 0},
          "ptx_vector_kernel_scalar_index_adds": {"p50": 0},
          "cuda_ptx_source_cache_entries": {"p50": 1},
          "cuda_ptx_source_cache_hits": {"p50": 1},
          "cuda_ptx_source_cache_misses": {"p50": 1}
        }"#;

        assert_eq!(
            validate_cuda_ptx_pattern_evidence(artifact)
                .expect_err("scalarized vector kernel should fail"),
            CudaPtxPatternEvidenceError::ThresholdMiss {
                metric: "ptx_vector_kernel_scalar_loads",
                observed: 1,
                required: 0,
            }
        );
    }
}

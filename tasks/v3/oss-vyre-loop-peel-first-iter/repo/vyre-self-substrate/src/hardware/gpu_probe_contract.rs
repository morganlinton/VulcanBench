//! GPU probe and no-skip test contract validation.

/// Observed GPU test/probe outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuProbeRecord<'a> {
    /// Test or gate name.
    pub gate: &'a str,
    /// Probe command or API used.
    pub probe: &'a str,
    /// Probe output or failure detail.
    pub detail: &'a str,
    /// Whether GPU was discovered.
    pub gpu_discovered: bool,
    /// Whether the gate skipped execution.
    pub skipped: bool,
}

/// GPU probe contract proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuProbeContractProof {
    /// Number of probe records.
    pub record_count: usize,
    /// Number of successful GPU discoveries.
    pub discovered_count: usize,
}

/// Committed GPU probe/no-fallback artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuProbeArtifactProof {
    /// Number of committed source artifacts validated.
    pub artifact_count: usize,
    /// Number of loud `Fix:` diagnostics in the validated artifacts.
    pub fix_diagnostic_count: usize,
    /// Number of CUDA/NVIDIA probe tokens in the validated artifacts.
    pub cuda_probe_token_count: usize,
}

/// GPU probe contract validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuProbeContractError {
    /// No probe records supplied.
    EmptyRecords,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Gate name.
        gate: String,
        /// Field.
        field: &'static str,
    },
    /// GPU test skipped instead of failing loudly.
    SkippedGpuGate {
        /// Gate name.
        gate: String,
    },
    /// Failed probe lacks adapter/device detail.
    MissingProbeFailureDetail {
        /// Gate name.
        gate: String,
    },
    /// Committed GPU probe artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed GPU probe artifact missed a required threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for GpuProbeContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "GPU probe contract has no records. Fix: every GPU gate must record adapter/device probe evidence."
            ),
            Self::EmptyMetadata { gate, field } => write!(
                f,
                "GPU probe record `{gate}` has empty {field}. Fix: record gate, probe, and discovery detail."
            ),
            Self::SkippedGpuGate { gate } => write!(
                f,
                "GPU gate `{gate}` skipped. Fix: fail loudly with probe detail instead of treating GPU absence as normal."
            ),
            Self::MissingProbeFailureDetail { gate } => write!(
                f,
                "GPU gate `{gate}` failed discovery without adapter/device detail. Fix: report nvidia-smi, CUDA device count, or adapter enumeration output."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "GPU probe artifact is missing {evidence}. Fix: prove committed tests fail loudly on GPU probe/configuration failures."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "GPU probe artifact {field}={observed} missed required {required}. Fix: restore broad CUDA/NVIDIA probe and no-skip coverage."
            ),
        }
    }
}

impl std::error::Error for GpuProbeContractError {}

/// Validate GPU probes and reject skip-on-no-GPU behavior.
pub fn validate_gpu_probe_contract(
    records: &[GpuProbeRecord<'_>],
) -> Result<GpuProbeContractProof, GpuProbeContractError> {
    if records.is_empty() {
        return Err(GpuProbeContractError::EmptyRecords);
    }
    let mut discovered_count = 0_usize;

    for record in records {
        for (field, value) in [
            ("gate", record.gate),
            ("probe", record.probe),
            ("detail", record.detail),
        ] {
            if value.trim().is_empty() {
                return Err(GpuProbeContractError::EmptyMetadata {
                    gate: record.gate.to_owned(),
                    field,
                });
            }
        }
        if record.skipped {
            return Err(GpuProbeContractError::SkippedGpuGate {
                gate: record.gate.to_owned(),
            });
        }
        if record.gpu_discovered {
            discovered_count += 1;
        } else if !has_probe_detail(record.detail) {
            return Err(GpuProbeContractError::MissingProbeFailureDetail {
                gate: record.gate.to_owned(),
            });
        }
    }

    Ok(GpuProbeContractProof {
        record_count: records.len(),
        discovered_count,
    })
}

fn has_probe_detail(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("nvidia-smi")
        || lower.contains("cuda")
        || lower.contains("adapter")
        || lower.contains("device")
}

/// Validate committed GPU probe and no-hidden-fallback source artifacts.
pub fn validate_gpu_probe_artifacts(
    gpu_boundary_contracts: &str,
    loudness_workspace_test: &str,
    hidden_fallback_workspace_test: &str,
    cuda_capability_contracts: &str,
    wgpu_no_cpu_fallback_test: &str,
    wgpu_dispatch_no_fallback_test: &str,
    release_matrix_source: &str,
) -> Result<GpuProbeArtifactProof, GpuProbeContractError> {
    for (artifact, evidence, needle) in [
        (
            gpu_boundary_contracts,
            "live nvidia-smi release-host probe",
            "release_host_must_expose_nvidia_gpu",
        ),
        (
            gpu_boundary_contracts,
            "NVIDIA adapter assertion",
            "stdout.contains(\"NVIDIA\")",
        ),
        (
            gpu_boundary_contracts,
            "adjacent consumer source coverage",
            "REQUIRED_ADJACENT_SOURCE_ROOTS",
        ),
        (
            gpu_boundary_contracts,
            "production CPU helper scan",
            "production_sources_do_not_call_cpu_helpers_outside_oracles",
        ),
        (
            loudness_workspace_test,
            "workspace loud GPU test",
            "gpu_required_tests_fail_loudly_instead_of_silently_skipping",
        ),
        (
            loudness_workspace_test,
            "silent GPU skip detector",
            "is_silent_gpu_skip",
        ),
        (
            loudness_workspace_test,
            "unsupported feature None detector",
            "production_paths_do_not_convert_unsupported_gpu_features_into_none",
        ),
        (
            loudness_workspace_test,
            "loud probe token masking regression",
            "loud_probe_token_does_not_mask_silent_gpu_skip_later_in_file",
        ),
        (
            hidden_fallback_workspace_test,
            "production fallback source scan",
            "production_sources_do_not_expose_hidden_cpu_fallbacks",
        ),
        (
            hidden_fallback_workspace_test,
            "CUDA production scan root",
            "vyre-driver-cuda/src",
        ),
        (
            hidden_fallback_workspace_test,
            "Dataflow sibling production scan root",
            "workspace.join(\"../../../dataflow\")",
        ),
        (
            hidden_fallback_workspace_test,
            "forbidden fallback phrase list",
            "FORBIDDEN_PATTERNS",
        ),
        (
            cuda_capability_contracts,
            "CUDA visible-device count probe",
            "visible_device_count",
        ),
        (
            cuda_capability_contracts,
            "CUDA probe_all coverage",
            "probe_all",
        ),
        (
            cuda_capability_contracts,
            "CUDA backend acquire must succeed",
            "CudaBackend::acquire",
        ),
        (
            cuda_capability_contracts,
            "CUDA preferred backend assertion",
            "preferred_dispatch_backend_is_cuda_not_cpu_or_wgpu_fallback",
        ),
        (
            cuda_capability_contracts,
            "CUDA no host grid sync split",
            "allows_host_grid_sync_split",
        ),
        (
            wgpu_no_cpu_fallback_test,
            "WGPU no fake skip paths",
            "no_fake_gpu_skip_paths_in_tests",
        ),
        (
            wgpu_no_cpu_fallback_test,
            "WGPU rejects CPU adapter",
            "DeviceType::Cpu",
        ),
        (
            wgpu_no_cpu_fallback_test,
            "WGPU actionable backend error",
            "backend_error_on_missing_gpu_is_actionable",
        ),
        (
            wgpu_dispatch_no_fallback_test,
            "dispatch rejects CPU demotion contract",
            "Dispatch-level CPU demotion rejection tests",
        ),
        (
            wgpu_dispatch_no_fallback_test,
            "dispatch consistent with GPU round trip",
            "dispatch_takes_gpu_consistent_time_not_instant",
        ),
        (
            wgpu_dispatch_no_fallback_test,
            "compile_native real GPU pipeline",
            "compile_native_returns_real_gpu_pipeline",
        ),
        (
            release_matrix_source,
            "GPU probe gate command",
            "vyre-gpu-probe-contract",
        ),
        (
            release_matrix_source,
            "GPU probe gate requires hardware probe",
            "requires_gpu_probe: true",
        ),
        (
            release_matrix_source,
            "CUDA gates require probe audit",
            "release_matrix_requires_gpu_probe_for_every_cuda_gate",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let artifacts = [
        gpu_boundary_contracts,
        loudness_workspace_test,
        hidden_fallback_workspace_test,
        cuda_capability_contracts,
        wgpu_no_cpu_fallback_test,
        wgpu_dispatch_no_fallback_test,
        release_matrix_source,
    ];
    let joined_len = artifacts
        .iter()
        .map(|artifact| artifact.len())
        .sum::<usize>();
    artifact_at_least("artifact source bytes", joined_len, 30_000)?;

    let fix_diagnostic_count = artifacts
        .iter()
        .map(|artifact| artifact.matches("Fix:").count())
        .sum();
    let cuda_probe_token_count = artifacts
        .iter()
        .map(|artifact| {
            artifact.matches("CUDA").count()
                + artifact.matches("cuda").count()
                + artifact.matches("nvidia-smi").count()
                + artifact.matches("NVIDIA").count()
        })
        .sum();

    artifact_at_least("Fix diagnostics", fix_diagnostic_count, 40)?;
    artifact_at_least("CUDA/NVIDIA probe tokens", cuda_probe_token_count, 40)?;

    Ok(GpuProbeArtifactProof {
        artifact_count: artifacts.len(),
        fix_diagnostic_count,
        cuda_probe_token_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), GpuProbeContractError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(GpuProbeContractError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), GpuProbeContractError> {
    if observed >= required {
        Ok(())
    } else {
        Err(GpuProbeContractError::ArtifactThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_probe_contract_accepts_discovered_gpu_records() {
        let proof = validate_gpu_probe_contract(&[GpuProbeRecord {
            gate: "cuda parity",
            probe: "nvidia-smi",
            detail: "NVIDIA GeForce RTX 5090 CUDA device 0",
            gpu_discovered: true,
            skipped: false,
        }])
        .expect("Fix: discovered GPU record should pass");

        assert_eq!(proof.record_count, 1);
        assert_eq!(proof.discovered_count, 1);
    }

    #[test]
    fn gpu_probe_contract_rejects_skipped_gpu_tests() {
        assert_eq!(
            validate_gpu_probe_contract(&[GpuProbeRecord {
                gate: "cuda parity",
                probe: "nvidia-smi",
                detail: "disabled: no GPU",
                gpu_discovered: false,
                skipped: true,
            }])
            .expect_err("skip-on-no-GPU must fail"),
            GpuProbeContractError::SkippedGpuGate {
                gate: "cuda parity".to_owned(),
            }
        );
    }

    #[test]
    fn gpu_probe_contract_requires_failure_detail() {
        assert_eq!(
            validate_gpu_probe_contract(&[GpuProbeRecord {
                gate: "cuda parity",
                probe: "nvidia-smi",
                detail: "not available",
                gpu_discovered: false,
                skipped: false,
            }])
            .expect_err("missing device detail should fail"),
            GpuProbeContractError::MissingProbeFailureDetail {
                gate: "cuda parity".to_owned(),
            }
        );
    }

    #[test]
    fn gpu_probe_contract_accepts_committed_loud_probe_artifacts() {
        let proof = committed_artifact_proof()
            .expect("Fix: committed GPU probe/fallback artifacts should prove loud probe behavior");

        assert_eq!(proof.artifact_count, 7);
        assert!(proof.fix_diagnostic_count >= 40);
        assert!(proof.cuda_probe_token_count >= 40);
    }

    #[test]
    fn gpu_probe_contract_rejects_missing_live_nvidia_probe() {
        let gpu_boundary_contracts =
            include_str!("../../../vyre-core/tests/gpu_boundary_contracts.rs").replace(
                "release_host_must_expose_nvidia_gpu",
                "release_host_probe_removed",
            );

        assert_eq!(
            validate_gpu_probe_artifacts(
                &gpu_boundary_contracts,
                include_str!("../../../vyre-foundation/tests/gpu_test_loudness_workspace.rs"),
                include_str!("../../../vyre-foundation/tests/no_hidden_cpu_fallback_workspace.rs"),
                include_str!("../../../vyre-driver-cuda/tests/capability_contracts.rs"),
                include_str!("../../../vyre-driver-wgpu/tests/no_cpu_fallback.rs"),
                include_str!("../../../vyre-driver-wgpu/tests/dispatch_never_cpu_fallback.rs"),
                include_str!("../integration/release/release_validation_matrix.rs"),
            )
            .expect_err("missing live nvidia-smi probe must fail"),
            GpuProbeContractError::ArtifactMissingEvidence {
                evidence: "live nvidia-smi release-host probe",
            }
        );
    }

    #[test]
    fn gpu_probe_contract_rejects_missing_cuda_preferred_backend_proof() {
        let cuda_capability_contracts =
            include_str!("../../../vyre-driver-cuda/tests/capability_contracts.rs").replace(
                "preferred_dispatch_backend_is_cuda_not_cpu_or_wgpu_fallback",
                "preferred_dispatch_backend_removed",
            );

        assert_eq!(
            validate_gpu_probe_artifacts(
                include_str!("../../../vyre-core/tests/gpu_boundary_contracts.rs"),
                include_str!("../../../vyre-foundation/tests/gpu_test_loudness_workspace.rs"),
                include_str!("../../../vyre-foundation/tests/no_hidden_cpu_fallback_workspace.rs"),
                &cuda_capability_contracts,
                include_str!("../../../vyre-driver-wgpu/tests/no_cpu_fallback.rs"),
                include_str!("../../../vyre-driver-wgpu/tests/dispatch_never_cpu_fallback.rs"),
                include_str!("../integration/release/release_validation_matrix.rs"),
            )
            .expect_err("missing CUDA preferred backend proof must fail"),
            GpuProbeContractError::ArtifactMissingEvidence {
                evidence: "CUDA preferred backend assertion",
            }
        );
    }

    fn committed_artifact_proof() -> Result<GpuProbeArtifactProof, GpuProbeContractError> {
        validate_gpu_probe_artifacts(
            include_str!("../../../vyre-core/tests/gpu_boundary_contracts.rs"),
            include_str!("../../../vyre-foundation/tests/gpu_test_loudness_workspace.rs"),
            include_str!("../../../vyre-foundation/tests/no_hidden_cpu_fallback_workspace.rs"),
            include_str!("../../../vyre-driver-cuda/tests/capability_contracts.rs"),
            include_str!("../../../vyre-driver-wgpu/tests/no_cpu_fallback.rs"),
            include_str!("../../../vyre-driver-wgpu/tests/dispatch_never_cpu_fallback.rs"),
            include_str!("../integration/release/release_validation_matrix.rs"),
        )
    }
}

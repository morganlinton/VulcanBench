//! Release GPU evidence validation.

/// GPU evidence captured for a release validation run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseGpuEvidence<'a> {
    /// Exact GPU probe command.
    pub probe_command: &'a str,
    /// Raw GPU probe output.
    pub probe_output: &'a str,
    /// Exact validation or benchmark command.
    pub validation_command: &'a str,
    /// Driver version parsed or copied from the probe.
    pub driver_version: &'a str,
    /// GPU model parsed or copied from the probe.
    pub gpu_model: &'a str,
}

/// Committed CUDA megakernel benchmark artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseCudaMegakernelArtifactProof {
    /// Number of CUDA backend case markers.
    pub cuda_case_count: usize,
    /// Number of pass status markers.
    pub pass_count: usize,
    /// Number of GPU-required case markers.
    pub gpu_required_count: usize,
}

/// Committed full CUDA release suite proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseCudaSuiteArtifactProof {
    /// CUDA workload family count.
    pub family_count: u64,
    /// CUDA workload artifact count.
    pub artifact_count: usize,
    /// CPU-SOTA 100x passing case count.
    pub hundred_x_passing_cases: usize,
}

/// Release GPU evidence errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseGpuEvidenceError {
    /// Required evidence field is empty.
    EmptyField {
        /// Field name.
        field: &'static str,
    },
    /// Probe command is not nvidia-smi.
    MissingNvidiaSmiProbe {
        /// Probe command.
        command: String,
    },
    /// Probe output does not identify NVIDIA/RTX CUDA hardware.
    MissingCudaHardware {
        /// Probe output.
        output: String,
    },
    /// Driver version is absent or not found in probe output.
    MissingDriverVersion {
        /// Driver version.
        driver_version: String,
    },
    /// Validation command does not use cargo_full.
    ValidationCommandDoesNotUseCargoFull {
        /// Validation command.
        command: String,
    },
}

impl std::fmt::Display for ReleaseGpuEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField { field } => write!(
                f,
                "release GPU evidence has empty {field}. Fix: record probe command, probe output, driver version, GPU model, and validation command."
            ),
            Self::MissingNvidiaSmiProbe { command } => write!(
                f,
                "release GPU evidence probe `{command}` is not nvidia-smi. Fix: record explicit nvidia-smi output before claiming CUDA validation."
            ),
            Self::MissingCudaHardware { output } => write!(
                f,
                "release GPU evidence output `{output}` does not identify NVIDIA/RTX CUDA hardware. Fix: fail the environment instead of silently treating CUDA as unavailable."
            ),
            Self::MissingDriverVersion { driver_version } => write!(
                f,
                "release GPU evidence driver version `{driver_version}` is not present in probe output. Fix: record exact driver output with the validation artifact."
            ),
            Self::ValidationCommandDoesNotUseCargoFull { command } => write!(
                f,
                "release GPU evidence validation command `{command}` does not use ./cargo_full. Fix: run release validation through cargo_full."
            ),
        }
    }
}

impl std::error::Error for ReleaseGpuEvidenceError {}

/// Committed CUDA megakernel benchmark artifact errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseCudaMegakernelArtifactError {
    /// Required literal field is missing.
    MissingField {
        /// Missing field.
        field: &'static str,
    },
    /// Required occurrence count is too low.
    ThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed count.
        observed: usize,
        /// Required count.
        required: usize,
    },
}

/// Full CUDA release suite artifact errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseCudaSuiteArtifactError {
    /// Required field is missing.
    MissingField {
        /// Missing field.
        field: &'static str,
    },
    /// Numeric threshold was missed.
    ThresholdMiss {
        /// Field name.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for ReleaseCudaMegakernelArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "CUDA megakernel artifact is missing {field}. Fix: record a real CUDA release benchmark artifact with GPU probe, pass status, and contract evidence."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "CUDA megakernel artifact {field} count {observed} missed required {required}. Fix: rerun the CUDA release benchmark suite and commit complete case evidence."
            ),
        }
    }
}

impl std::error::Error for ReleaseCudaMegakernelArtifactError {}

impl std::fmt::Display for ReleaseCudaSuiteArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                f,
                "CUDA release suite artifact is missing {field}. Fix: record backend matrix and full CUDA workload suite evidence with RTX 5090 probe data."
            ),
            Self::ThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "CUDA release suite artifact {field}={observed} missed required {required}. Fix: rerun the full CUDA benchmark suite and commit complete family evidence."
            ),
        }
    }
}

impl std::error::Error for ReleaseCudaSuiteArtifactError {}

/// Validate that a release artifact contains concrete CUDA hardware evidence.
pub fn validate_release_gpu_evidence(
    evidence: ReleaseGpuEvidence<'_>,
) -> Result<(), ReleaseGpuEvidenceError> {
    for (field, value) in [
        ("probe_command", evidence.probe_command),
        ("probe_output", evidence.probe_output),
        ("validation_command", evidence.validation_command),
        ("driver_version", evidence.driver_version),
        ("gpu_model", evidence.gpu_model),
    ] {
        if value.trim().is_empty() {
            return Err(ReleaseGpuEvidenceError::EmptyField { field });
        }
    }
    if !evidence.probe_command.contains("nvidia-smi") {
        return Err(ReleaseGpuEvidenceError::MissingNvidiaSmiProbe {
            command: evidence.probe_command.to_owned(),
        });
    }
    if !evidence.probe_output.contains("NVIDIA")
        && !evidence.probe_output.contains("RTX")
        && !evidence.probe_output.contains("CUDA")
    {
        return Err(ReleaseGpuEvidenceError::MissingCudaHardware {
            output: evidence.probe_output.to_owned(),
        });
    }
    if !evidence.probe_output.contains(evidence.driver_version)
        || evidence
            .driver_version
            .chars()
            .filter(|ch| *ch == '.')
            .count()
            < 1
    {
        return Err(ReleaseGpuEvidenceError::MissingDriverVersion {
            driver_version: evidence.driver_version.to_owned(),
        });
    }
    if !evidence
        .validation_command
        .trim_start()
        .starts_with("./cargo_full ")
    {
        return Err(
            ReleaseGpuEvidenceError::ValidationCommandDoesNotUseCargoFull {
                command: evidence.validation_command.to_owned(),
            },
        );
    }
    Ok(())
}

/// Validate the committed CUDA megakernel benchmark artifact.
pub fn validate_release_cuda_megakernel_artifact(
    artifact: &str,
) -> Result<ReleaseCudaMegakernelArtifactProof, ReleaseCudaMegakernelArtifactError> {
    cuda_artifact_contains(
        artifact,
        "selected CUDA backend",
        "\"selected_backend\": \"cuda\"",
    )?;
    cuda_artifact_contains(artifact, "GPU environment", "\"has_gpu\": true")?;
    cuda_artifact_contains(
        artifact,
        "RTX CUDA hardware",
        "\"name\": \"NVIDIA GeForce RTX 5090\"",
    )?;
    cuda_artifact_contains(
        artifact,
        "NVIDIA driver",
        "\"nvidia_driver_version\": \"570.211.01\"",
    )?;
    cuda_artifact_contains(
        artifact,
        "CUDA runtime version",
        "\"nvidia_cuda_version\": \"12.8\"",
    )?;
    cuda_artifact_contains(artifact, "usable CUDA backend", "\"backend.usable.cuda\"")?;
    cuda_artifact_contains(
        artifact,
        "release megakernel case",
        "\"release.megakernel_queue.1m\"",
    )?;
    cuda_artifact_contains(
        artifact,
        "persistent megakernel contract",
        "\"persistent megakernel queued condition batches\"",
    )?;
    cuda_artifact_contains(artifact, "exact correctness", "\"correctness\": \"Exact\"")?;
    cuda_artifact_contains(
        artifact,
        "100x CPU SOTA contract",
        "\"min_speedup_x\": 100.0",
    )?;
    cuda_artifact_contains(artifact, "contract passed", "\"contract_passed\": true")?;
    cuda_artifact_contains(artifact, "no performance violations", "\"violations\": []")?;

    let cuda_case_count = artifact.matches("\"backend_id\": \"cuda\"").count();
    let pass_count = artifact.matches("\"status\": \"pass\"").count();
    let gpu_required_count = artifact.matches("\"needs_gpu\": true").count();

    cuda_artifact_at_least("backend_id=cuda", cuda_case_count, 1)?;
    cuda_artifact_at_least("status=pass", pass_count, 1)?;
    cuda_artifact_at_least("needs_gpu=true", gpu_required_count, 1)?;

    Ok(ReleaseCudaMegakernelArtifactProof {
        cuda_case_count,
        pass_count,
        gpu_required_count,
    })
}

/// Validate committed backend matrix and full CUDA benchmark-suite artifacts.
pub fn validate_release_cuda_suite_artifacts(
    backend_matrix: &str,
    cuda_suite: &str,
) -> Result<ReleaseCudaSuiteArtifactProof, ReleaseCudaSuiteArtifactError> {
    for (artifact, field, needle) in [
        (
            backend_matrix,
            "backend matrix schema",
            "\"schema_version\": 2",
        ),
        (backend_matrix, "CUDA-first flag", "\"cuda_first\": true"),
        (
            backend_matrix,
            "preferred CUDA backend",
            "\"preferred_backend_id\": \"cuda\"",
        ),
        (
            backend_matrix,
            "GPU-only backend preference",
            "\"preferred_backend_gpu_only\": true",
        ),
        (
            backend_matrix,
            "nvidia-smi probe success",
            "\"nvidia_smi_ok\": true",
        ),
        (
            backend_matrix,
            "RTX 5090 probe device",
            "NVIDIA GeForce RTX 5090",
        ),
        (
            backend_matrix,
            "NVIDIA driver version",
            "\"nvidia_driver_version\": \"570.211.01\"",
        ),
        (
            backend_matrix,
            "CUDA runtime version",
            "\"nvidia_cuda_version\": \"12.8\"",
        ),
        (
            backend_matrix,
            "CUDA PTX source cache marker",
            "\"id\": \"cuda-ptx-source-cache\"",
        ),
        (
            backend_matrix,
            "CUDA resident dispatch marker",
            "\"id\": \"cuda-resident-dispatch\"",
        ),
        (
            backend_matrix,
            "CUDA graph launch marker",
            "\"id\": \"cuda-graph-launch\"",
        ),
        (cuda_suite, "CUDA suite schema", "\"schema_version\": 2"),
        (cuda_suite, "CUDA suite backend", "\"backend\": \"cuda\""),
        (cuda_suite, "CUDA family count", "\"family_count\": 16"),
        (
            cuda_suite,
            "RTX 5090 suite hardware",
            "\"gpu_model\": \"NVIDIA GeForce RTX 5090\"",
        ),
        (
            cuda_suite,
            "CUDA suite driver",
            "\"nvidia_driver_version\": \"570.211.01\"",
        ),
        (
            cuda_suite,
            "CUDA suite runtime",
            "\"nvidia_cuda_version\": \"12.8\"",
        ),
        (cuda_suite, "zero failed cases", "\"failed_count\": 0"),
        (
            cuda_suite,
            "zero backend mismatches",
            "\"nonmatching_case_backend_count\": 0",
        ),
        (cuda_suite, "empty blockers", "\"blockers\": []"),
        (
            cuda_suite,
            "queued megakernel family",
            "\"family_id\": \"megakernel-queued-batches\"",
        ),
        (
            cuda_suite,
            "callgraph reachability family",
            "\"family_id\": \"callgraph-reachability\"",
        ),
    ] {
        suite_artifact_contains(artifact, field, needle)?;
    }

    let artifact_count = cuda_suite
        .matches("release/evidence/benchmarks/workload-")
        .count();
    let selected_cuda_count = cuda_suite.matches("\"selected_backend\": \"cuda\"").count();
    let wall_sample_count = cuda_suite.matches("\"min_wall_samples\": 30").count();
    let baseline_sample_count = cuda_suite
        .matches("\"min_baseline_wall_samples\": 30")
        .count();
    let hundred_x_required_count = cuda_suite
        .matches("\"cpu_sota_100x_required\": true")
        .count();
    let hundred_x_passing_cases = cuda_suite
        .matches("\"cpu_sota_100x_passing_cases\": 1")
        .count();

    suite_artifact_at_least("workload artifact rows", artifact_count, 13)?;
    suite_artifact_at_least("selected_backend=cuda rows", selected_cuda_count, 13)?;
    suite_artifact_at_least("min_wall_samples=30 rows", wall_sample_count, 13)?;
    suite_artifact_at_least(
        "min_baseline_wall_samples=30 rows",
        baseline_sample_count,
        13,
    )?;
    suite_artifact_at_least("cpu_sota_100x_required rows", hundred_x_required_count, 10)?;
    suite_artifact_at_least(
        "cpu_sota_100x_passing_cases rows",
        hundred_x_passing_cases,
        10,
    )?;

    Ok(ReleaseCudaSuiteArtifactProof {
        family_count: 16,
        artifact_count,
        hundred_x_passing_cases,
    })
}

fn cuda_artifact_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), ReleaseCudaMegakernelArtifactError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ReleaseCudaMegakernelArtifactError::MissingField { field })
    }
}

fn cuda_artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), ReleaseCudaMegakernelArtifactError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ReleaseCudaMegakernelArtifactError::ThresholdMiss {
            field,
            observed,
            required,
        })
    }
}

fn suite_artifact_contains(
    artifact: &str,
    field: &'static str,
    needle: &str,
) -> Result<(), ReleaseCudaSuiteArtifactError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(ReleaseCudaSuiteArtifactError::MissingField { field })
    }
}

fn suite_artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), ReleaseCudaSuiteArtifactError> {
    if observed >= required {
        Ok(())
    } else {
        Err(ReleaseCudaSuiteArtifactError::ThresholdMiss {
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
    fn gpu_evidence_accepts_nvidia_smi_and_cargo_full_commands() {
        validate_release_gpu_evidence(evidence()).expect("Fix: valid GPU evidence should pass");
    }

    #[test]
    fn gpu_evidence_rejects_missing_probe_and_cpu_hardware() {
        let mut bad = evidence();
        bad.probe_command = "true";
        assert_eq!(
            validate_release_gpu_evidence(bad).expect_err("missing nvidia-smi should fail"),
            ReleaseGpuEvidenceError::MissingNvidiaSmiProbe {
                command: "true".to_owned(),
            }
        );

        let mut cpu = evidence();
        cpu.probe_output = "generic CPU";
        assert_eq!(
            validate_release_gpu_evidence(cpu).expect_err("CPU-only evidence should fail"),
            ReleaseGpuEvidenceError::MissingCudaHardware {
                output: "generic CPU".to_owned(),
            }
        );
    }

    #[test]
    fn gpu_evidence_rejects_driver_or_command_mismatch() {
        let mut driver = evidence();
        driver.driver_version = "999.0";
        assert_eq!(
            validate_release_gpu_evidence(driver).expect_err("missing driver should fail"),
            ReleaseGpuEvidenceError::MissingDriverVersion {
                driver_version: "999.0".to_owned(),
            }
        );

        let mut command = evidence();
        command.validation_command = "cargo test";
        assert_eq!(
            validate_release_gpu_evidence(command).expect_err("raw cargo should fail"),
            ReleaseGpuEvidenceError::ValidationCommandDoesNotUseCargoFull {
                command: "cargo test".to_owned(),
            }
        );
    }

    #[test]
    fn gpu_evidence_accepts_committed_cuda_megakernel_artifact() {
        let proof = validate_release_cuda_megakernel_artifact(include_str!(
            "../../../../release/evidence/benchmarks/megakernel-condition-cuda.json"
        ))
        .expect("Fix: committed CUDA megakernel release artifact should pass");

        assert!(proof.cuda_case_count >= 1);
        assert!(proof.pass_count >= 1);
        assert!(proof.gpu_required_count >= 1);
    }

    #[test]
    fn gpu_evidence_accepts_committed_full_cuda_suite_artifacts() {
        let proof = validate_release_cuda_suite_artifacts(
            include_str!("../../../../release/evidence/backends/backend-matrix.json"),
            include_str!("../../../../release/evidence/benchmarks/cuda-release-suite.json"),
        )
        .expect("Fix: committed backend matrix and CUDA suite should pass");

        assert_eq!(proof.family_count, 16);
        assert!(proof.artifact_count >= 13);
        assert!(proof.hundred_x_passing_cases >= 10);
    }

    #[test]
    fn gpu_evidence_rejects_partial_cuda_suite_artifact() {
        let err = validate_release_cuda_suite_artifacts(
            include_str!("../../../../release/evidence/backends/backend-matrix.json"),
            r#"{
              "schema_version": 2,
              "backend": "cuda",
              "family_count": 1,
              "gpu_model": "NVIDIA GeForce RTX 5090",
              "nvidia_driver_version": "570.211.01",
              "nvidia_cuda_version": "12.8",
              "failed_count": 0,
              "nonmatching_case_backend_count": 0,
              "blockers": []
            }"#,
        )
        .expect_err("partial CUDA suite must not satisfy full release evidence");

        assert_eq!(
            err,
            ReleaseCudaSuiteArtifactError::MissingField {
                field: "CUDA family count",
            }
        );
    }

    #[test]
    fn gpu_evidence_rejects_wgpu_or_cpu_megakernel_artifact() {
        let artifact = r#"{
          "selected_backend": "wgpu",
          "has_gpu": true,
          "name": "NVIDIA GeForce RTX 5090",
          "nvidia_driver_version": "570.211.01",
          "nvidia_cuda_version": "12.8",
          "features": ["backend.usable.cuda"],
          "cases": [{
            "id": "release.megakernel_queue.1m",
            "backend_id": "cuda",
            "needs_gpu": true,
            "status": "pass",
            "correctness": "Exact",
            "contract": {
              "primitive": "persistent megakernel queued condition batches",
              "baselines": [{"min_speedup_x": 100.0}]
            },
            "performance": {
              "contract_passed": true,
              "violations": []
            }
          }]
        }"#;

        assert_eq!(
            validate_release_cuda_megakernel_artifact(artifact)
                .expect_err("non-CUDA selected backend should fail"),
            ReleaseCudaMegakernelArtifactError::MissingField {
                field: "selected CUDA backend",
            }
        );
    }

    #[test]
    fn gpu_evidence_rejects_missing_cuda_case_status() {
        let artifact = r#"{
          "selected_backend": "cuda",
          "has_gpu": true,
          "name": "NVIDIA GeForce RTX 5090",
          "nvidia_driver_version": "570.211.01",
          "nvidia_cuda_version": "12.8",
          "features": ["backend.usable.cuda"],
          "id": "release.megakernel_queue.1m",
          "primitive": "persistent megakernel queued condition batches",
          "correctness": "Exact",
          "min_speedup_x": 100.0,
          "contract_passed": true,
          "violations": []
        }"#;

        assert_eq!(
            validate_release_cuda_megakernel_artifact(artifact)
                .expect_err("artifact without a CUDA case should fail"),
            ReleaseCudaMegakernelArtifactError::ThresholdMiss {
                field: "backend_id=cuda",
                observed: 0,
                required: 1,
            }
        );
    }

    fn evidence() -> ReleaseGpuEvidence<'static> {
        ReleaseGpuEvidence {
            probe_command: "nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap --format=csv,noheader",
            probe_output: "NVIDIA GeForce RTX 5090, 570.211.01, 32607 MiB, 12.0",
            validation_command: "./cargo_full test -j1 -p vyre-driver-cuda",
            driver_version: "570.211.01",
            gpu_model: "NVIDIA GeForce RTX 5090",
        }
    }
}

//! Release benchmark baseline validation.

use std::collections::BTreeSet;

/// Direction used when comparing an observed metric to its release threshold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkThresholdDirection {
    /// Observed value must be greater than or equal to the threshold.
    AtLeast,
    /// Observed value must be less than or equal to the threshold.
    AtMost,
}

/// One committed benchmark baseline record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReleaseBenchmarkBaseline {
    /// Stable baseline id.
    pub id: &'static str,
    /// Exact command used to produce the observation.
    pub command: &'static str,
    /// Hardware label, including GPU model.
    pub hardware: &'static str,
    /// Metric name.
    pub metric: &'static str,
    /// Observed metric value.
    pub observed: f64,
    /// Release threshold.
    pub threshold: f64,
    /// Threshold comparison direction.
    pub direction: BenchmarkThresholdDirection,
}

/// Validated benchmark baseline summary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReleaseBenchmarkBaselineProof {
    /// Number of accepted baselines.
    pub baseline_count: usize,
    /// Number of CUDA baselines.
    pub cuda_baseline_count: usize,
}

/// One committed benchmark artifact checked by the release baseline gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReleaseBenchmarkArtifact<'a> {
    /// Repository-relative artifact path.
    pub path: &'a str,
    /// Raw artifact contents.
    pub contents: &'a str,
    /// Human-readable workload family that must appear in the artifact.
    pub required_family: &'a str,
}

/// Validated committed benchmark artifact summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseBenchmarkArtifactProof {
    /// Number of accepted committed artifacts.
    pub artifact_count: usize,
}

/// Benchmark baseline validation errors.
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseBenchmarkBaselineError {
    /// No baselines provided.
    EmptyBaselines,
    /// Baseline id is duplicate.
    DuplicateId {
        /// Duplicate id.
        id: &'static str,
    },
    /// Required metadata is empty.
    EmptyMetadata {
        /// Baseline id.
        id: &'static str,
        /// Empty field name.
        field: &'static str,
    },
    /// Command does not use cargo_full.
    CommandDoesNotUseCargoFull {
        /// Baseline id.
        id: &'static str,
        /// Command.
        command: &'static str,
    },
    /// Hardware does not name a CUDA GPU.
    MissingCudaHardware {
        /// Baseline id.
        id: &'static str,
        /// Hardware label.
        hardware: &'static str,
    },
    /// Observed or threshold value is invalid.
    InvalidMetric {
        /// Baseline id.
        id: &'static str,
    },
    /// Observed value misses the release threshold.
    ThresholdMiss {
        /// Baseline id.
        id: &'static str,
        /// Observed value.
        observed: f64,
        /// Threshold value.
        threshold: f64,
        /// Direction.
        direction: BenchmarkThresholdDirection,
    },
    /// A committed benchmark artifact is missing required release evidence.
    ArtifactMissingEvidence {
        /// Artifact path.
        path: String,
        /// Missing evidence field.
        evidence: &'static str,
    },
}

impl std::fmt::Display for ReleaseBenchmarkBaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBaselines => write!(
                f,
                "release benchmark baselines are empty. Fix: commit exact benchmark command, CUDA hardware, metric, observed value, and threshold."
            ),
            Self::DuplicateId { id } => write!(
                f,
                "release benchmark baseline `{id}` is duplicated. Fix: keep one owner per release metric."
            ),
            Self::EmptyMetadata { id, field } => write!(
                f,
                "release benchmark baseline `{id}` has empty {field}. Fix: every baseline needs id, command, hardware, metric, observed value, and threshold."
            ),
            Self::CommandDoesNotUseCargoFull { id, command } => write!(
                f,
                "release benchmark baseline `{id}` uses `{command}` instead of ./cargo_full. Fix: record the exact cargo_full benchmark command."
            ),
            Self::MissingCudaHardware { id, hardware } => write!(
                f,
                "release benchmark baseline `{id}` hardware `{hardware}` does not name CUDA/NVIDIA hardware. Fix: record the RTX 5090/CUDA device used for the run."
            ),
            Self::InvalidMetric { id } => write!(
                f,
                "release benchmark baseline `{id}` has invalid metric values. Fix: observed and threshold must be positive finite values."
            ),
            Self::ThresholdMiss {
                id,
                observed,
                threshold,
                direction,
            } => write!(
                f,
                "release benchmark baseline `{id}` missed threshold: observed={observed}, threshold={threshold}, direction={direction:?}. Fix: improve performance or update the release target with explicit approval."
            ),
            Self::ArtifactMissingEvidence { path, evidence } => write!(
                f,
                "release benchmark artifact `{path}` is missing {evidence}. Fix: commit benchmark evidence with CUDA hardware, pass status, 100x contract, samples, and source fingerprint."
            ),
        }
    }
}

impl std::error::Error for ReleaseBenchmarkBaselineError {}

/// Validate committed release benchmark baselines.
pub fn validate_release_benchmark_baselines(
    baselines: &[ReleaseBenchmarkBaseline],
) -> Result<ReleaseBenchmarkBaselineProof, ReleaseBenchmarkBaselineError> {
    if baselines.is_empty() {
        return Err(ReleaseBenchmarkBaselineError::EmptyBaselines);
    }
    let mut ids = BTreeSet::new();
    let mut cuda_baseline_count = 0_usize;

    for baseline in baselines {
        validate_metadata(baseline)?;
        if !ids.insert(baseline.id) {
            return Err(ReleaseBenchmarkBaselineError::DuplicateId { id: baseline.id });
        }
        if !baseline.command.trim_start().starts_with("./cargo_full ") {
            return Err(ReleaseBenchmarkBaselineError::CommandDoesNotUseCargoFull {
                id: baseline.id,
                command: baseline.command,
            });
        }
        if !baseline.hardware.contains("CUDA")
            && !baseline.hardware.contains("NVIDIA")
            && !baseline.hardware.contains("RTX")
        {
            return Err(ReleaseBenchmarkBaselineError::MissingCudaHardware {
                id: baseline.id,
                hardware: baseline.hardware,
            });
        }
        cuda_baseline_count += 1;
        if !baseline.observed.is_finite()
            || !baseline.threshold.is_finite()
            || baseline.observed <= 0.0
            || baseline.threshold <= 0.0
        {
            return Err(ReleaseBenchmarkBaselineError::InvalidMetric { id: baseline.id });
        }
        let passed = match baseline.direction {
            BenchmarkThresholdDirection::AtLeast => baseline.observed >= baseline.threshold,
            BenchmarkThresholdDirection::AtMost => baseline.observed <= baseline.threshold,
        };
        if !passed {
            return Err(ReleaseBenchmarkBaselineError::ThresholdMiss {
                id: baseline.id,
                observed: baseline.observed,
                threshold: baseline.threshold,
                direction: baseline.direction,
            });
        }
    }

    Ok(ReleaseBenchmarkBaselineProof {
        baseline_count: baselines.len(),
        cuda_baseline_count,
    })
}

/// Validate committed benchmark artifacts, not just in-code baseline records.
pub fn validate_committed_benchmark_artifacts(
    artifacts: &[ReleaseBenchmarkArtifact<'_>],
) -> Result<ReleaseBenchmarkArtifactProof, ReleaseBenchmarkBaselineError> {
    if artifacts.is_empty() {
        return Err(ReleaseBenchmarkBaselineError::EmptyBaselines);
    }
    for artifact in artifacts {
        validate_artifact_field(artifact, "path", artifact.path)?;
        validate_artifact_field(artifact, "contents", artifact.contents)?;
        validate_artifact_field(artifact, "required_family", artifact.required_family)?;
        require_artifact_contains(
            artifact,
            "selected CUDA backend",
            "\"selected_backend\": \"cuda\"",
        )?;
        require_artifact_contains(artifact, "RTX 5090 hardware", "NVIDIA GeForce RTX 5090")?;
        require_artifact_contains(
            artifact,
            "NVIDIA driver version",
            "\"nvidia_driver_version\"",
        )?;
        require_artifact_contains(artifact, "source fingerprint", "\"source_fingerprint\"")?;
        require_artifact_contains_any(
            artifact,
            "passing case status",
            &["\"status\": \"pass\"", "\"failed_count\": 0"],
        )?;
        require_artifact_contains_any(
            artifact,
            "100x speedup floor",
            &["\"min_speedup_x\": 100", "\"cpu_sota_100x_required\": true"],
        )?;
        require_artifact_contains_any(
            artifact,
            "passed performance contract",
            &[
                "\"contract_passed\": true",
                "\"cpu_sota_100x_passing_cases\": 1",
            ],
        )?;
        require_artifact_contains_any(
            artifact,
            "wall sample count",
            &["\"samples\"", "\"min_wall_samples\""],
        )?;
        if !artifact.contents.contains(artifact.required_family) {
            return Err(ReleaseBenchmarkBaselineError::ArtifactMissingEvidence {
                path: artifact.path.to_owned(),
                evidence: "required workload family",
            });
        }
    }
    Ok(ReleaseBenchmarkArtifactProof {
        artifact_count: artifacts.len(),
    })
}

fn validate_artifact_field(
    artifact: &ReleaseBenchmarkArtifact<'_>,
    field: &'static str,
    value: &str,
) -> Result<(), ReleaseBenchmarkBaselineError> {
    if value.trim().is_empty() {
        return Err(ReleaseBenchmarkBaselineError::ArtifactMissingEvidence {
            path: artifact.path.to_owned(),
            evidence: field,
        });
    }
    Ok(())
}

fn require_artifact_contains(
    artifact: &ReleaseBenchmarkArtifact<'_>,
    evidence: &'static str,
    needle: &str,
) -> Result<(), ReleaseBenchmarkBaselineError> {
    if artifact.contents.contains(needle) {
        Ok(())
    } else {
        Err(ReleaseBenchmarkBaselineError::ArtifactMissingEvidence {
            path: artifact.path.to_owned(),
            evidence,
        })
    }
}

fn require_artifact_contains_any(
    artifact: &ReleaseBenchmarkArtifact<'_>,
    evidence: &'static str,
    needles: &[&str],
) -> Result<(), ReleaseBenchmarkBaselineError> {
    if needles
        .iter()
        .any(|needle| artifact.contents.contains(needle))
    {
        Ok(())
    } else {
        Err(ReleaseBenchmarkBaselineError::ArtifactMissingEvidence {
            path: artifact.path.to_owned(),
            evidence,
        })
    }
}

fn validate_metadata(
    baseline: &ReleaseBenchmarkBaseline,
) -> Result<(), ReleaseBenchmarkBaselineError> {
    for (field, value) in [
        ("id", baseline.id),
        ("command", baseline.command),
        ("hardware", baseline.hardware),
        ("metric", baseline.metric),
    ] {
        if value.trim().is_empty() {
            return Err(ReleaseBenchmarkBaselineError::EmptyMetadata {
                id: baseline.id,
                field,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_baselines_accept_exact_cuda_cargo_full_records() {
        let proof = validate_release_benchmark_baselines(&[
            baseline(
                "cuda-megakernel-100x",
                125.0,
                100.0,
                BenchmarkThresholdDirection::AtLeast,
            ),
            baseline(
                "cuda-readback-us",
                42.0,
                50.0,
                BenchmarkThresholdDirection::AtMost,
            ),
        ])
        .expect("Fix: valid CUDA baselines should pass");

        assert_eq!(proof.baseline_count, 2);
        assert_eq!(proof.cuda_baseline_count, 2);
    }

    #[test]
    fn benchmark_baselines_reject_non_cargo_full_commands() {
        let mut bad = baseline(
            "bad-command",
            125.0,
            100.0,
            BenchmarkThresholdDirection::AtLeast,
        );
        bad.command = "cargo bench";

        assert_eq!(
            validate_release_benchmark_baselines(&[bad]).expect_err("raw cargo bench should fail"),
            ReleaseBenchmarkBaselineError::CommandDoesNotUseCargoFull {
                id: "bad-command",
                command: "cargo bench",
            }
        );
    }

    #[test]
    fn benchmark_baselines_reject_missing_cuda_hardware_and_threshold_miss() {
        let mut cpu = baseline("cpu", 125.0, 100.0, BenchmarkThresholdDirection::AtLeast);
        cpu.hardware = "generic x86";
        assert_eq!(
            validate_release_benchmark_baselines(&[cpu])
                .expect_err("missing CUDA hardware should fail"),
            ReleaseBenchmarkBaselineError::MissingCudaHardware {
                id: "cpu",
                hardware: "generic x86",
            }
        );

        assert_eq!(
            validate_release_benchmark_baselines(&[baseline(
                "too-slow",
                99.0,
                100.0,
                BenchmarkThresholdDirection::AtLeast,
            )])
            .expect_err("threshold miss should fail"),
            ReleaseBenchmarkBaselineError::ThresholdMiss {
                id: "too-slow",
                observed: 99.0,
                threshold: 100.0,
                direction: BenchmarkThresholdDirection::AtLeast,
            }
        );
    }

    #[test]
    fn benchmark_artifacts_accept_committed_cuda_release_evidence() {
        let proof = validate_committed_benchmark_artifacts(&[
            artifact(
                "release/evidence/benchmarks/cuda-release-suite.json",
                include_str!("../../../../release/evidence/benchmarks/cuda-release-suite.json"),
                "ifds-witness",
            ),
            artifact(
                "release/evidence/benchmarks/megakernel-condition-cuda.json",
                include_str!("../../../../release/evidence/benchmarks/megakernel-condition-cuda.json"),
                "megakernel",
            ),
            artifact(
                "release/evidence/benchmarks/workload-10-megakernel-queued-batches.json",
                include_str!(
                    "../../../../release/evidence/benchmarks/workload-10-megakernel-queued-batches.json"
                ),
                "megakernel",
            ),
            artifact(
                "release/evidence/benchmarks/dataflow-analysis-release.json",
                include_str!("../../../../release/evidence/benchmarks/dataflow-analysis-release.json"),
                "dataflow",
            ),
        ])
        .expect("Fix: committed CUDA benchmark artifacts should satisfy release evidence contracts");

        assert_eq!(proof.artifact_count, 4);
    }

    #[test]
    fn benchmark_artifacts_reject_missing_100x_contract() {
        let err = validate_committed_benchmark_artifacts(&[artifact(
            "bad.json",
            "{\"selected_backend\": \"cuda\", \"gpu\": \"NVIDIA GeForce RTX 5090\", \"nvidia_driver_version\": \"570.211.01\", \"source_fingerprint\": \"git:x\", \"status\": \"pass\", \"contract_passed\": true, \"samples\": 35, \"family\": \"megakernel\"}",
            "megakernel",
        )])
        .expect_err("missing 100x contract should fail");

        assert_eq!(
            err,
            ReleaseBenchmarkBaselineError::ArtifactMissingEvidence {
                path: "bad.json".to_owned(),
                evidence: "100x speedup floor",
            }
        );
    }

    fn artifact(
        path: &'static str,
        contents: &'static str,
        required_family: &'static str,
    ) -> ReleaseBenchmarkArtifact<'static> {
        ReleaseBenchmarkArtifact {
            path,
            contents,
            required_family,
        }
    }

    fn baseline(
        id: &'static str,
        observed: f64,
        threshold: f64,
        direction: BenchmarkThresholdDirection,
    ) -> ReleaseBenchmarkBaseline {
        ReleaseBenchmarkBaseline {
            id,
            command: "./cargo_full bench -j1 -p vyre-driver-cuda",
            hardware: "NVIDIA RTX 5090 CUDA",
            metric: "speedup_x",
            observed,
            threshold,
            direction,
        }
    }
}

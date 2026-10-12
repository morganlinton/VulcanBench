//! Hot-loop allocation regression validation.

/// One hot-loop allocation sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationRegressionSample<'a> {
    /// Hot-loop label.
    pub loop_name: &'a str,
    /// Iterations measured after warmup.
    pub iterations: u64,
    /// Host allocations observed after warmup.
    pub post_warmup_allocations: u64,
    /// Device allocations observed after warmup.
    pub post_warmup_device_allocations: u64,
    /// Output slot capacity before repeated execution.
    pub output_capacity_before: u64,
    /// Output slot capacity after repeated execution.
    pub output_capacity_after: u64,
}

/// Allocation regression proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationRegressionProof {
    /// Number of accepted samples.
    pub sample_count: usize,
    /// Total post-warmup iterations covered.
    pub total_iterations: u64,
}

/// Committed allocation-regression evidence proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationRegressionArtifactProof {
    /// CUDA benchmark artifact count validated.
    pub cuda_artifact_count: usize,
}

/// Allocation regression errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllocationRegressionError {
    /// No samples were supplied.
    EmptySamples,
    /// Required metadata is empty.
    EmptyLoopName,
    /// Iterations must be non-zero.
    ZeroIterations {
        /// Hot-loop label.
        loop_name: String,
    },
    /// Host allocation occurred after warmup.
    HostAllocationAfterWarmup {
        /// Hot-loop label.
        loop_name: String,
        /// Allocation count.
        allocations: u64,
    },
    /// Device allocation occurred after warmup.
    DeviceAllocationAfterWarmup {
        /// Hot-loop label.
        loop_name: String,
        /// Allocation count.
        allocations: u64,
    },
    /// Output capacity changed across repeated execution.
    OutputCapacityChanged {
        /// Hot-loop label.
        loop_name: String,
        /// Capacity before.
        before: u64,
        /// Capacity after.
        after: u64,
    },
    /// Committed evidence is missing required allocation-regression proof.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
}

impl std::fmt::Display for AllocationRegressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySamples => write!(
                f,
                "allocation regression samples are empty. Fix: record hot-loop host/device allocation counters after warmup."
            ),
            Self::EmptyLoopName => write!(
                f,
                "allocation regression sample has empty loop_name. Fix: name every measured hot loop."
            ),
            Self::ZeroIterations { loop_name } => write!(
                f,
                "allocation regression sample `{loop_name}` has zero iterations. Fix: measure repeated post-warmup execution."
            ),
            Self::HostAllocationAfterWarmup {
                loop_name,
                allocations,
            } => write!(
                f,
                "hot loop `{loop_name}` allocated {allocations} host object(s) after warmup. Fix: move allocation to setup or reuse caller-owned slots."
            ),
            Self::DeviceAllocationAfterWarmup {
                loop_name,
                allocations,
            } => write!(
                f,
                "hot loop `{loop_name}` allocated {allocations} device object(s) after warmup. Fix: use resident pools or preplanned scratch."
            ),
            Self::OutputCapacityChanged {
                loop_name,
                before,
                after,
            } => write!(
                f,
                "hot loop `{loop_name}` changed output capacity from {before} to {after}. Fix: preserve caller-owned output slots across repeated execution."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "allocation regression committed evidence is missing {evidence}. Fix: preserve CUDA scratch reuse, compact readback, and before-allocation overflow rejection evidence."
            ),
        }
    }
}

impl std::error::Error for AllocationRegressionError {}

/// Validate hot-loop allocation samples.
pub fn validate_allocation_regression(
    samples: &[AllocationRegressionSample<'_>],
) -> Result<AllocationRegressionProof, AllocationRegressionError> {
    if samples.is_empty() {
        return Err(AllocationRegressionError::EmptySamples);
    }
    let mut total_iterations = 0_u64;
    for sample in samples {
        if sample.loop_name.trim().is_empty() {
            return Err(AllocationRegressionError::EmptyLoopName);
        }
        if sample.iterations == 0 {
            return Err(AllocationRegressionError::ZeroIterations {
                loop_name: sample.loop_name.to_owned(),
            });
        }
        if sample.post_warmup_allocations != 0 {
            return Err(AllocationRegressionError::HostAllocationAfterWarmup {
                loop_name: sample.loop_name.to_owned(),
                allocations: sample.post_warmup_allocations,
            });
        }
        if sample.post_warmup_device_allocations != 0 {
            return Err(AllocationRegressionError::DeviceAllocationAfterWarmup {
                loop_name: sample.loop_name.to_owned(),
                allocations: sample.post_warmup_device_allocations,
            });
        }
        if sample.output_capacity_before != sample.output_capacity_after {
            return Err(AllocationRegressionError::OutputCapacityChanged {
                loop_name: sample.loop_name.to_owned(),
                before: sample.output_capacity_before,
                after: sample.output_capacity_after,
            });
        }
        total_iterations = total_iterations.saturating_add(sample.iterations);
    }

    Ok(AllocationRegressionProof {
        sample_count: samples.len(),
        total_iterations,
    })
}

/// Validate committed allocation-regression artifacts and source contracts.
pub fn validate_allocation_regression_artifacts(
    cuda_benchmark_artifacts: &[&str],
    cuda_csr_source: &str,
    runtime_scratch_source: &str,
    allocation_bounds_source: &str,
) -> Result<AllocationRegressionArtifactProof, AllocationRegressionError> {
    if cuda_benchmark_artifacts.is_empty() {
        return Err(AllocationRegressionError::EmptySamples);
    }
    for artifact in cuda_benchmark_artifacts {
        for (evidence, needle) in [
            ("CUDA backend benchmark", "\"selected_backend\": \"cuda\""),
            ("RTX benchmark hardware", "NVIDIA GeForce RTX 5090"),
            ("allocation byte metrics", "\"alloc_bytes\""),
            ("allocation count metrics", "\"alloc_count\""),
            ("benchmark samples", "\"samples\""),
            ("passing benchmark status", "\"status\": \"pass\""),
            ("source fingerprint", "\"source_fingerprint\""),
        ] {
            artifact_contains(artifact, evidence, needle)?;
        }
    }

    for (evidence, needle) in [
        (
            "resident CSR queue API test",
            "cuda_resident_csr_queue_api_reuses_graph_and_scratch",
        ),
        ("caller-owned scratch", "ResidentCsrQueueScratch::default"),
        ("caller-owned output capacity", "Vec::with_capacity"),
        (
            "output capacity preserved",
            "preserve caller-owned output capacity",
        ),
        ("scratch resident slot reuse", "resident_query_slots"),
        (
            "frontier payload capacity reuse",
            "frontier_payload_capacity",
        ),
        ("compact readback assertion", "compact readback"),
        (
            "graph resident reuse assertion",
            "keep CSR graph state resident",
        ),
    ] {
        artifact_contains(cuda_csr_source, evidence, needle)?;
    }

    for (evidence, needle) in [
        (
            "rule catalog scratch test",
            "pack_rule_catalog_into_reuses_caller_storage",
        ),
        ("rule meta pointer stability", "rule_meta.as_ptr()"),
        ("transition pointer stability", "transitions.as_ptr()"),
        ("accept pointer stability", "accept.as_ptr()"),
        ("rejection pointer stability", "rejected_rules.as_ptr()"),
        ("repeated packing success", "repeated packing must succeed"),
    ] {
        artifact_contains(runtime_scratch_source, evidence, needle)?;
    }

    for (evidence, needle) in [
        (
            "allocation bounds test module",
            "Unbounded allocation rejection",
        ),
        ("overflow rejected before allocation", "before allocation"),
        (
            "debug log overflow rejection",
            "try_encode_empty_debug_log_rejects_overflowing_record_capacity",
        ),
        (
            "queue overflow rejection",
            "u32::MAX io queue must be rejected before allocation",
        ),
    ] {
        artifact_contains(allocation_bounds_source, evidence, needle)?;
    }

    Ok(AllocationRegressionArtifactProof {
        cuda_artifact_count: cuda_benchmark_artifacts.len(),
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), AllocationRegressionError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(AllocationRegressionError::ArtifactMissingEvidence { evidence })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_regression_accepts_zero_allocation_hot_loops() {
        let proof = validate_allocation_regression(&[
            sample("resident-csr-batch", 1_000, 0, 0, 4096, 4096),
            sample("frontier-megakernel", 2_000, 0, 0, 8192, 8192),
        ])
        .expect("Fix: zero allocation samples should pass");

        assert_eq!(proof.sample_count, 2);
        assert_eq!(proof.total_iterations, 3_000);
    }

    #[test]
    fn allocation_regression_rejects_host_and_device_allocations() {
        assert_eq!(
            validate_allocation_regression(&[sample("hot", 1, 1, 0, 8, 8)])
                .expect_err("host allocation should fail"),
            AllocationRegressionError::HostAllocationAfterWarmup {
                loop_name: "hot".to_owned(),
                allocations: 1,
            }
        );
        assert_eq!(
            validate_allocation_regression(&[sample("hot", 1, 0, 1, 8, 8)])
                .expect_err("device allocation should fail"),
            AllocationRegressionError::DeviceAllocationAfterWarmup {
                loop_name: "hot".to_owned(),
                allocations: 1,
            }
        );
    }

    #[test]
    fn allocation_regression_rejects_output_capacity_growth() {
        assert_eq!(
            validate_allocation_regression(&[sample("hot", 1, 0, 0, 8, 16)])
                .expect_err("capacity growth should fail"),
            AllocationRegressionError::OutputCapacityChanged {
                loop_name: "hot".to_owned(),
                before: 8,
                after: 16,
            }
        );
    }

    #[test]
    fn allocation_regression_accepts_committed_cuda_artifacts_and_source_contracts() {
        let proof = validate_allocation_regression_artifacts(
            &[
                include_str!("../../../../release/evidence/benchmarks/megakernel-condition-cuda.json"),
                include_str!(
                    "../../../../release/evidence/benchmarks/workload-10-megakernel-queued-batches.json"
                ),
                include_str!("../../../../release/evidence/benchmarks/dataflow-analysis-release.json"),
            ],
            // The CSR-frontier-queue parity tests were split out of the parent
            // file into per-concern submodules; concatenate the ones that carry
            // the resident-CSR-queue API + caller-owned-scratch contracts so the
            // source evidence is read where it now lives.
            &format!(
                "{}\n{}",
                include_str!(
                    "../../../../vyre-driver-cuda/tests/csr_frontier_queue_gpu_parity/resident_graph_contracts.rs"
                ),
                include_str!(
                    "../../../../vyre-driver-cuda/tests/csr_frontier_queue_gpu_parity/batch_contracts.rs"
                ),
            ),
            include_str!("../../../../vyre-runtime/tests/megakernel_rule_catalog_scratch.rs"),
            include_str!("../../../../vyre-runtime/tests/megakernel_allocation_bounds.rs"),
        )
        .expect("Fix: committed allocation regression artifacts should pass");

        assert_eq!(proof.cuda_artifact_count, 3);
    }

    #[test]
    fn allocation_regression_rejects_missing_scratch_reuse_source() {
        let err = validate_allocation_regression_artifacts(
            &[include_str!(
                "../../../../release/evidence/benchmarks/megakernel-condition-cuda.json"
            )],
            "fn unrelated() {}",
            include_str!("../../../../vyre-runtime/tests/megakernel_rule_catalog_scratch.rs"),
            include_str!("../../../../vyre-runtime/tests/megakernel_allocation_bounds.rs"),
        )
        .expect_err("missing resident scratch reuse source should fail");

        assert_eq!(
            err,
            AllocationRegressionError::ArtifactMissingEvidence {
                evidence: "resident CSR queue API test",
            }
        );
    }

    fn sample<'a>(
        loop_name: &'a str,
        iterations: u64,
        host_allocations: u64,
        device_allocations: u64,
        output_capacity_before: u64,
        output_capacity_after: u64,
    ) -> AllocationRegressionSample<'a> {
        AllocationRegressionSample {
            loop_name,
            iterations,
            post_warmup_allocations: host_allocations,
            post_warmup_device_allocations: device_allocations,
            output_capacity_before,
            output_capacity_after,
        }
    }
}

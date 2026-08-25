//! Host/device memory ownership contract validation.

use std::collections::BTreeSet;

/// Allowed owner for a buffer at a system boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOwner {
    /// Caller owns the host-visible input/output allocation.
    HostCaller,
    /// CUDA owns a resident device allocation.
    DeviceResident,
    /// Runtime owns pinned staging for transfers only.
    PinnedStaging,
    /// Caller owns output slots reused across dispatches.
    BorrowedOutputSlot,
    /// CPU/reference memory exists only inside parity tests.
    ParityOnly,
}

/// One buffer ownership declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryOwnershipRecord<'a> {
    /// Stable buffer or resource name.
    pub resource: &'a str,
    /// Owning subsystem.
    pub subsystem: &'a str,
    /// Declared memory owner.
    pub owner: MemoryOwner,
    /// Whether this record is for production code.
    pub production: bool,
}

/// Memory ownership proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryOwnershipProof {
    /// Number of ownership records validated.
    pub record_count: usize,
    /// Number of device-resident records.
    pub device_resident_count: usize,
    /// Number of borrowed output-slot records.
    pub borrowed_output_slot_count: usize,
}

/// Committed memory-ownership source and release artifact proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryOwnershipArtifactProof {
    /// Number of committed source/artifact surfaces validated.
    pub surface_count: usize,
    /// Number of output-slot reuse tokens found.
    pub output_reuse_token_count: usize,
    /// Number of CUDA resident ownership tokens found.
    pub resident_token_count: usize,
}

/// Memory ownership validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryOwnershipError {
    /// No ownership records were supplied.
    EmptyRecords,
    /// Required metadata is empty.
    EmptyMetadata {
        /// Resource name.
        resource: String,
        /// Field name.
        field: &'static str,
    },
    /// Resource is declared more than once.
    DuplicateResource {
        /// Resource name.
        resource: String,
    },
    /// Parity-only memory is production-visible.
    ParityOnlyInProduction {
        /// Resource name.
        resource: String,
    },
    /// Release contract lacks device-resident ownership evidence.
    MissingDeviceResident,
    /// Release contract lacks borrowed output-slot evidence.
    MissingBorrowedOutputSlots,
    /// Committed memory ownership artifact is missing required evidence.
    ArtifactMissingEvidence {
        /// Missing evidence.
        evidence: &'static str,
    },
    /// Committed memory ownership artifact missed a required threshold.
    ArtifactThresholdMiss {
        /// Field.
        field: &'static str,
        /// Observed value.
        observed: usize,
        /// Required value.
        required: usize,
    },
}

impl std::fmt::Display for MemoryOwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRecords => write!(
                f,
                "memory ownership contract has no records. Fix: declare host caller, device resident, pinned staging, borrowed output, and parity-only boundaries."
            ),
            Self::EmptyMetadata { resource, field } => write!(
                f,
                "memory ownership record `{resource}` has empty {field}. Fix: every resource needs a subsystem and owner."
            ),
            Self::DuplicateResource { resource } => write!(
                f,
                "memory ownership resource `{resource}` is declared more than once. Fix: choose one owner and route other users through that contract."
            ),
            Self::ParityOnlyInProduction { resource } => write!(
                f,
                "memory ownership resource `{resource}` is parity-only but production-visible. Fix: move CPU/reference memory behind a test-only boundary."
            ),
            Self::MissingDeviceResident => write!(
                f,
                "memory ownership contract has no device-resident resources. Fix: CUDA release paths must declare resident device ownership explicitly."
            ),
            Self::MissingBorrowedOutputSlots => write!(
                f,
                "memory ownership contract has no borrowed output slots. Fix: repeated dispatch outputs must use caller-owned reusable slots."
            ),
            Self::ArtifactMissingEvidence { evidence } => write!(
                f,
                "memory ownership artifact is missing {evidence}. Fix: prove the real driver/CUDA sources use one resident/staging/output ownership model."
            ),
            Self::ArtifactThresholdMiss {
                field,
                observed,
                required,
            } => write!(
                f,
                "memory ownership artifact {field}={observed} missed required {required}. Fix: restore resident CUDA ownership and caller-owned output reuse evidence."
            ),
        }
    }
}

impl std::error::Error for MemoryOwnershipError {}

/// Validate host/device memory ownership records.
pub fn validate_memory_ownership_contract(
    records: &[MemoryOwnershipRecord<'_>],
) -> Result<MemoryOwnershipProof, MemoryOwnershipError> {
    if records.is_empty() {
        return Err(MemoryOwnershipError::EmptyRecords);
    }

    let mut resources = BTreeSet::new();
    let mut device_resident_count = 0_usize;
    let mut borrowed_output_slot_count = 0_usize;

    for record in records {
        for (field, value) in [
            ("resource", record.resource),
            ("subsystem", record.subsystem),
        ] {
            if value.trim().is_empty() {
                return Err(MemoryOwnershipError::EmptyMetadata {
                    resource: record.resource.to_owned(),
                    field,
                });
            }
        }
        if !resources.insert(record.resource) {
            return Err(MemoryOwnershipError::DuplicateResource {
                resource: record.resource.to_owned(),
            });
        }
        if record.production && record.owner == MemoryOwner::ParityOnly {
            return Err(MemoryOwnershipError::ParityOnlyInProduction {
                resource: record.resource.to_owned(),
            });
        }
        match record.owner {
            MemoryOwner::DeviceResident => device_resident_count += 1,
            MemoryOwner::BorrowedOutputSlot => borrowed_output_slot_count += 1,
            MemoryOwner::HostCaller | MemoryOwner::PinnedStaging | MemoryOwner::ParityOnly => {}
        }
    }

    if device_resident_count == 0 {
        return Err(MemoryOwnershipError::MissingDeviceResident);
    }
    if borrowed_output_slot_count == 0 {
        return Err(MemoryOwnershipError::MissingBorrowedOutputSlots);
    }

    Ok(MemoryOwnershipProof {
        record_count: records.len(),
        device_resident_count,
        borrowed_output_slot_count,
    })
}

/// Validate committed source and CUDA evidence for host/device memory ownership.
pub fn validate_memory_ownership_artifacts(
    backend_contract_source: &str,
    dispatch_result_source: &str,
    cuda_resident_source: &str,
    cuda_resident_io_source: &str,
    cuda_allocations_source: &str,
    backend_matrix: &str,
    cuda_release_suite: &str,
) -> Result<MemoryOwnershipArtifactProof, MemoryOwnershipError> {
    for (artifact, evidence, needle) in [
        (
            backend_contract_source,
            "driver DeviceBuffer contract",
            "DeviceBuffer",
        ),
        (
            backend_contract_source,
            "driver HostShimBuffer contract",
            "HostShimBuffer",
        ),
        (
            backend_contract_source,
            "driver output buffer contract",
            "OutputBuffers",
        ),
        (
            backend_contract_source,
            "driver output-slot preservation export",
            "replace_output_buffers_preserving_slots",
        ),
        (
            dispatch_result_source,
            "caller-owned output buffer alias",
            "pub type OutputBuffers = Vec<Vec<u8>>",
        ),
        (
            dispatch_result_source,
            "output-slot reuse function",
            "replace_output_buffers_preserving_slots_with_memory_stats",
        ),
        (
            dispatch_result_source,
            "reused output-slot accounting",
            "reused_slots",
        ),
        (
            dispatch_result_source,
            "moved output-slot accounting",
            "moved_slots",
        ),
        (
            dispatch_result_source,
            "retained capacity accounting",
            "retained_capacity_bytes",
        ),
        (
            cuda_resident_source,
            "CUDA resident buffer handle",
            "pub struct CudaResidentBuffer",
        ),
        (
            cuda_resident_source,
            "CUDA resident store owner",
            "struct CudaResidentStore",
        ),
        (
            cuda_resident_source,
            "resident byte budget reservation",
            "reserve_resident_budget",
        ),
        (
            cuda_resident_source,
            "resident inflight guard",
            "mark_inflight",
        ),
        (
            cuda_resident_source,
            "resident unknown-handle diagnostic",
            "not owned by this backend",
        ),
        (
            cuda_resident_io_source,
            "CUDA resident allocation API",
            "allocate_resident",
        ),
        (
            cuda_resident_io_source,
            "batched resident upload API",
            "upload_resident_many",
        ),
        (
            cuda_resident_io_source,
            "caller-owned resident download API",
            "download_resident_into",
        ),
        (
            cuda_resident_io_source,
            "batched sparse readback API",
            "download_resident_readbacks_many",
        ),
        (
            cuda_resident_io_source,
            "resident readback byte accounting",
            "record_device_to_host_readback",
        ),
        (
            cuda_allocations_source,
            "pinned host staging pool",
            "PinnedHostAllocationPool",
        ),
        (
            cuda_allocations_source,
            "bounded pinned staging cache",
            "max_cached_bytes",
        ),
        (
            cuda_allocations_source,
            "caller-owned copy into Vec",
            "copy_raw_bytes_into_vec",
        ),
        (
            backend_matrix,
            "CUDA-first backend matrix",
            "\"cuda_first\": true",
        ),
        (
            backend_matrix,
            "CUDA preferred backend",
            "\"preferred_backend_id\": \"cuda\"",
        ),
        (
            backend_matrix,
            "CUDA resident IO marker",
            "\"id\": \"cuda-resident-io\"",
        ),
        (
            backend_matrix,
            "CUDA resident dispatch marker",
            "\"id\": \"cuda-resident-dispatch\"",
        ),
        (
            backend_matrix,
            "no missing backend tokens",
            "\"missing_tokens\": []",
        ),
        (
            backend_matrix,
            "no unresolved backend markers",
            "\"unresolved_markers\": []",
        ),
        (
            cuda_release_suite,
            "CUDA release suite backend",
            "\"backend\": \"cuda\"",
        ),
        (
            cuda_release_suite,
            "RTX 5090 release suite hardware",
            "NVIDIA GeForce RTX 5090",
        ),
        (
            cuda_release_suite,
            "CUDA selected benchmark backend",
            "\"selected_backend\": \"cuda\"",
        ),
        (
            cuda_release_suite,
            "CUDA release suite zero blockers",
            "\"blockers\": []",
        ),
        (
            cuda_release_suite,
            "CUDA 100x contract",
            "\"cpu_sota_100x_required\": true",
        ),
    ] {
        artifact_contains(artifact, evidence, needle)?;
    }

    let output_reuse_token_count = dispatch_result_source
        .matches("replace_output_buffers")
        .count();
    let resident_token_count = cuda_resident_source.matches("CudaResident").count()
        + cuda_resident_io_source.matches("resident").count();

    artifact_at_least("output reuse tokens", output_reuse_token_count, 8)?;
    artifact_at_least("CUDA resident ownership tokens", resident_token_count, 40)?;

    Ok(MemoryOwnershipArtifactProof {
        surface_count: 7,
        output_reuse_token_count,
        resident_token_count,
    })
}

fn artifact_contains(
    artifact: &str,
    evidence: &'static str,
    needle: &str,
) -> Result<(), MemoryOwnershipError> {
    if artifact.contains(needle) {
        Ok(())
    } else {
        Err(MemoryOwnershipError::ArtifactMissingEvidence { evidence })
    }
}

fn artifact_at_least(
    field: &'static str,
    observed: usize,
    required: usize,
) -> Result<(), MemoryOwnershipError> {
    if observed >= required {
        Ok(())
    } else {
        Err(MemoryOwnershipError::ArtifactThresholdMiss {
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
    fn memory_ownership_accepts_cuda_release_boundaries() {
        let proof = validate_memory_ownership_contract(&[
            record("frontend-input", "vyrec", MemoryOwner::HostCaller, true),
            record(
                "resident-csr",
                "vyre-cuda",
                MemoryOwner::DeviceResident,
                true,
            ),
            record(
                "upload-stage",
                "vyre-cuda",
                MemoryOwner::PinnedStaging,
                true,
            ),
            record(
                "analysis-output",
                "vyre-cuda",
                MemoryOwner::BorrowedOutputSlot,
                true,
            ),
            record(
                "reference-oracle",
                "vyre-cuda-tests",
                MemoryOwner::ParityOnly,
                false,
            ),
        ])
        .expect("Fix: valid memory ownership contract should pass");

        assert_eq!(proof.record_count, 5);
        assert_eq!(proof.device_resident_count, 1);
        assert_eq!(proof.borrowed_output_slot_count, 1);
    }

    #[test]
    fn memory_ownership_rejects_parity_memory_in_production() {
        assert_eq!(
            validate_memory_ownership_contract(&[
                record(
                    "resident-csr",
                    "vyre-cuda",
                    MemoryOwner::DeviceResident,
                    true
                ),
                record(
                    "analysis-output",
                    "vyre-cuda",
                    MemoryOwner::BorrowedOutputSlot,
                    true,
                ),
                record("cpu-oracle", "vyre-cuda", MemoryOwner::ParityOnly, true),
            ])
            .expect_err("production parity-only memory should fail"),
            MemoryOwnershipError::ParityOnlyInProduction {
                resource: "cpu-oracle".to_owned(),
            }
        );
    }

    #[test]
    fn memory_ownership_requires_residency_and_borrowed_outputs() {
        assert_eq!(
            validate_memory_ownership_contract(&[record(
                "analysis-output",
                "vyre-cuda",
                MemoryOwner::BorrowedOutputSlot,
                true,
            )])
            .expect_err("missing device resident record should fail"),
            MemoryOwnershipError::MissingDeviceResident
        );
        assert_eq!(
            validate_memory_ownership_contract(&[record(
                "resident-csr",
                "vyre-cuda",
                MemoryOwner::DeviceResident,
                true,
            )])
            .expect_err("missing borrowed output slot should fail"),
            MemoryOwnershipError::MissingBorrowedOutputSlots
        );
    }

    #[test]
    fn memory_ownership_accepts_committed_driver_and_cuda_artifacts() {
        let proof = committed_artifact_proof()
            .expect("Fix: committed driver/CUDA sources should prove memory ownership");

        assert_eq!(proof.surface_count, 7);
        assert!(proof.output_reuse_token_count >= 8);
        assert!(proof.resident_token_count >= 40);
    }

    #[test]
    fn memory_ownership_rejects_missing_output_slot_reuse_source() {
        let dispatch_result_source =
            include_str!("../../../vyre-driver/src/backend/dispatch_result.rs").replace(
                "replace_output_buffers_preserving_slots_with_memory_stats",
                "replace_outputs",
            );

        assert_eq!(
            validate_memory_ownership_artifacts(
                include_str!("../../../vyre-driver/src/backend.rs"),
                &dispatch_result_source,
                include_str!("../../../vyre-driver-cuda/src/backend/resident.rs"),
                include_str!("../../../vyre-driver-cuda/src/backend/resident_io.rs"),
                include_str!("../../../vyre-driver-cuda/src/backend/allocations.rs"),
                include_str!("../../../release/evidence/backends/backend-matrix.json"),
                include_str!("../../../release/evidence/benchmarks/cuda-release-suite.json"),
            )
            .expect_err("missing output-slot preservation must fail"),
            MemoryOwnershipError::ArtifactMissingEvidence {
                evidence: "output-slot reuse function",
            }
        );
    }

    #[test]
    fn memory_ownership_rejects_non_cuda_release_artifact() {
        let cuda_release_suite =
            include_str!("../../../release/evidence/benchmarks/cuda-release-suite.json")
                .replace("\"backend\": \"cuda\"", "\"backend\": \"wgpu\"");

        assert_eq!(
            validate_memory_ownership_artifacts(
                include_str!("../../../vyre-driver/src/backend.rs"),
                include_str!("../../../vyre-driver/src/backend/dispatch_result.rs"),
                include_str!("../../../vyre-driver-cuda/src/backend/resident.rs"),
                include_str!("../../../vyre-driver-cuda/src/backend/resident_io.rs"),
                include_str!("../../../vyre-driver-cuda/src/backend/allocations.rs"),
                include_str!("../../../release/evidence/backends/backend-matrix.json"),
                &cuda_release_suite,
            )
            .expect_err("memory release proof must stay CUDA-backed"),
            MemoryOwnershipError::ArtifactMissingEvidence {
                evidence: "CUDA release suite backend",
            }
        );
    }

    fn record<'a>(
        resource: &'a str,
        subsystem: &'a str,
        owner: MemoryOwner,
        production: bool,
    ) -> MemoryOwnershipRecord<'a> {
        MemoryOwnershipRecord {
            resource,
            subsystem,
            owner,
            production,
        }
    }

    fn committed_artifact_proof() -> Result<MemoryOwnershipArtifactProof, MemoryOwnershipError> {
        validate_memory_ownership_artifacts(
            include_str!("../../../vyre-driver/src/backend.rs"),
            include_str!("../../../vyre-driver/src/backend/dispatch_result.rs"),
            include_str!("../../../vyre-driver-cuda/src/backend/resident.rs"),
            include_str!("../../../vyre-driver-cuda/src/backend/resident_io.rs"),
            include_str!("../../../vyre-driver-cuda/src/backend/allocations.rs"),
            include_str!("../../../release/evidence/backends/backend-matrix.json"),
            include_str!("../../../release/evidence/benchmarks/cuda-release-suite.json"),
        )
    }
}

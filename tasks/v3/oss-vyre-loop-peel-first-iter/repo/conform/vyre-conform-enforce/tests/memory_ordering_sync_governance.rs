//! Memory ordering sync governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const ORDERING: &str =
    include_str!("../../../docs/optimization/MEMORY_ORDERING_ATOMIC_CONTRACTS.toml");
const BARRIERS: &str = include_str!("../../../docs/optimization/GPU_BARRIER_SCOPE_MATRIX.toml");
const VISIBILITY: &str = include_str!("../../../docs/optimization/HOST_DEVICE_VISIBILITY_POLICY.toml");
const EVIDENCE: &str =
    include_str!("../../../docs/optimization/GPU_BARRIER_VERIFICATION_EVIDENCE.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_MEMORY_ORDERING_SYNCHRONIZATION_TRANCHE_COVERAGE.toml"
);

#[test]
fn memory_ordering_sync_sources_are_registered() {
    for key in [
        "RUST_ATOMIC_ORDERING",
        "RUST_NOMICON_ATOMICS",
        "CUDA_CPP_SYNCHRONIZATION",
        "WGSL_MEMORY_MODEL",
        "VULKAN_SYNC_CACHE_CONTROL",
        "VULKAN_MEMORY_MODEL",
        "METAL_RESOURCE_SYNCHRONIZATION",
        "METAL_FENCE_SYNCHRONIZATION",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn memory_ordering_contracts_record_writer_reader_atomic_non_atomic_model_and_counterexample_policies() {
    for required in [
        "contract_id",
        "shared_state",
        "writer_policy",
        "reader_policy",
        "atomic_order_policy",
        "non_atomic_data_policy",
        "failure_mode_policy",
        "model_check_policy",
        "counterexample_policy",
        "release_gate_effect",
        "resident-ring-slot-publication",
        "output-slab-readback-epoch",
        "autotune-cache-publication",
    ] {
        assert!(ORDERING.contains(required), "memory ordering contract must include {required}");
    }
}

#[test]
fn gpu_barrier_scope_matrix_records_backend_scope_storage_order_cross_workgroup_host_visibility_elision_and_capability() {
    for required in [
        "barrier_id",
        "backend_surface",
        "scope_policy",
        "storage_class_policy",
        "ordering_policy",
        "cross_workgroup_policy",
        "host_visibility_policy",
        "elision_policy",
        "capability_policy",
        "diagnostic_policy",
        "cuda-thread-block-shared-memory-barrier",
        "wgsl-workgroup-storage-barrier",
        "vulkan-queue-pipeline-barrier",
        "metal-pass-fence-resource-synchronization",
    ] {
        assert!(BARRIERS.contains(required), "GPU barrier scope matrix must include {required}");
    }
}

#[test]
fn host_device_visibility_policy_records_upload_readback_command_reuse_cache_stale_trace_and_gate_effects() {
    for required in [
        "visibility_id",
        "transfer_surface",
        "host_write_policy",
        "device_write_policy",
        "completion_policy",
        "cache_policy",
        "readback_policy",
        "stale_data_policy",
        "trace_policy",
        "release_gate_effect",
        "host-upload-device-read-visibility",
        "device-output-host-readback-visibility",
        "multi-queue-command-reuse-visibility",
    ] {
        assert!(VISIBILITY.contains(required), "host device visibility policy must include {required}");
    }
}

#[test]
fn barrier_verification_evidence_records_hazards_producers_consumers_negative_cases_counterexamples_and_cross_backend_policy() {
    for required in [
        "evidence_id",
        "barrier_contract",
        "hazard_class",
        "producer_policy",
        "consumer_policy",
        "negative_case_policy",
        "elision_counterexample_policy",
        "cross_backend_policy",
        "artifact_policy",
        "privacy_boundary",
        "shared-memory-producer-consumer-barrier-proof",
        "global-output-readback-fence-proof",
        "atomic-rmw-consistency-proof",
    ] {
        assert!(EVIDENCE.contains(required), "barrier verification evidence must include {required}");
    }
}

#[test]
fn plan_contains_memory_ordering_synchronization_rows() {
    for row in [
        "VX-1421",
        "VX-1422",
        "VX-1423",
        "VX-1424",
        "VX-1425",
        "VX-1426",
        "VX-1427",
        "VX-1428",
        "VX-1429",
        "VX-1430",
        "VX-1431",
        "VX-1432",
        "VX-1433",
        "VX-1434",
        "VX-1435",
        "VX-1436",
        "VX-1437",
        "VX-1438",
        "VX-1439",
        "VX-1440",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn memory_ordering_sync_coverage_reuses_formal_correctness_schedules_output_provenance_capabilities_telemetry_and_publication_authorities() {
    for required in [
        "VX-1421..VX-1440",
        "memory_ordering_atomic_contracts",
        "gpu_barrier_scope_matrix",
        "host_device_visibility_policy",
        "gpu_barrier_verification_evidence",
        "correctness_validation_coverage",
        "concurrency_schedule_contracts",
        "output_slab_provenance",
        "target_instruction_capabilities",
        "backend_capability_digests",
        "operator_evidence_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "memory ordering sync coverage must include {required}");
    }
}

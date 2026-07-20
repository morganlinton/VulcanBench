//! Memory residency transfer governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const TIERS: &str = include_str!("../../../docs/optimization/MEMORY_RESIDENCY_TIER_CONTRACTS.toml");
const STAGING: &str = include_str!("../../../docs/optimization/PINNED_STAGING_TRANSFER_POLICY.toml");
const PRESSURE: &str = include_str!("../../../docs/optimization/MEMORY_PRESSURE_OVERSUBSCRIPTION_RESPONSE.toml");
const OVERLAP: &str = include_str!("../../../docs/optimization/TRANSFER_OVERLAP_PIPELINE_EVIDENCE.toml");
const COVERAGE: &str = include_str!("../../../docs/optimization/END_TO_END_MEMORY_RESIDENCY_TRANSFER_TRANCHE_COVERAGE.toml");

#[test]
fn memory_residency_transfer_sources_are_registered() {
    for key in [
        "CUDA_UNIFIED_MEMORY",
        "CUDA_RUNTIME_MEMORY_API",
        "VULKAN_MEMORY_ALLOCATION",
        "WEBGPU_BUFFER_MAPPING",
        "METAL_RESOURCE_STORAGE_MODES",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn memory_residency_tiers_record_backend_policy_access_coherency_fault_pressure_and_links() {
    for required in [
        "tier_id",
        "backend_surface",
        "residency_policy",
        "host_access_policy",
        "device_access_policy",
        "coherency_policy",
        "page_fault_policy",
        "oversubscription_policy",
        "authority_links",
        "cuda-managed-prefetch-tier",
        "cuda-pinned-host-staging-tier",
        "vulkan-memory-heap-tier",
        "webgpu-staging-map-tier",
        "metal-storage-mode-tier",
    ] {
        assert!(TIERS.contains(required), "memory residency tier contract must include {required}");
    }
}

#[test]
fn pinned_staging_policy_records_pinning_alignment_zero_copy_overlap_budget_and_equivalence() {
    for required in [
        "transfer_id",
        "source_tier",
        "destination_tier",
        "pinning_policy",
        "alignment_policy",
        "zero_copy_policy",
        "async_overlap_policy",
        "budget_policy",
        "equivalence_policy",
        "scan-input-pinned-upload",
        "output-readback-pinned-download",
        "webgpu-metal-staging-upload",
    ] {
        assert!(STAGING.contains(required), "pinned staging transfer policy must include {required}");
    }
}

#[test]
fn memory_pressure_response_records_measurement_admission_migration_fallback_operator_privacy_and_diagnostics() {
    for required in [
        "pressure_id",
        "pressure_surface",
        "measurement_policy",
        "admission_policy",
        "migration_policy",
        "fallback_policy",
        "operator_effect_policy",
        "privacy_boundary",
        "cuda-managed-oversubscription-pressure",
        "vulkan-pageable-device-local-pressure",
        "staging-pool-pressure",
    ] {
        assert!(PRESSURE.contains(required), "memory pressure response must include {required}");
    }
}

#[test]
fn transfer_overlap_evidence_records_windows_counters_correctness_failures_and_authority_links() {
    for required in [
        "pipeline_id",
        "copy_surface",
        "producer_policy",
        "consumer_policy",
        "overlap_window_policy",
        "counter_policy",
        "correctness_policy",
        "failure_policy",
        "upload-compute-double-buffer-overlap",
        "managed-prefetch-compute-overlap",
        "device-output-readback-overlap",
    ] {
        assert!(OVERLAP.contains(required), "transfer overlap evidence must include {required}");
    }
}

#[test]
fn plan_contains_memory_residency_transfer_rows() {
    for row in [
        "VX-1461",
        "VX-1462",
        "VX-1463",
        "VX-1464",
        "VX-1465",
        "VX-1466",
        "VX-1467",
        "VX-1468",
        "VX-1469",
        "VX-1470",
        "VX-1471",
        "VX-1472",
        "VX-1473",
        "VX-1474",
        "VX-1475",
        "VX-1476",
        "VX-1477",
        "VX-1478",
        "VX-1479",
        "VX-1480",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn memory_residency_transfer_coverage_reuses_ingress_pool_budget_backpressure_capability_lifetime_visibility_output_profile_statistics_and_publication_authorities() {
    for required in [
        "VX-1461..VX-1480",
        "memory_residency_tier_contracts",
        "pinned_staging_transfer_policy",
        "memory_pressure_oversubscription_response",
        "transfer_overlap_pipeline_evidence",
        "corpus_paging_planner",
        "gpu_direct_corpus_paging_capabilities",
        "cuda_stream_ordered_pool_planner",
        "cuda_scan_memory_pools",
        "resource_budget_policy",
        "backpressure_queue_quota_policy",
        "backend_capability_digests",
        "backend_handle_lifetime_provenance",
        "host_device_visibility_policy",
        "output_slab_provenance",
        "profile_trace_metric_correlation_policy",
        "statistical_regression_gates",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "memory residency transfer coverage must include {required}");
    }
}

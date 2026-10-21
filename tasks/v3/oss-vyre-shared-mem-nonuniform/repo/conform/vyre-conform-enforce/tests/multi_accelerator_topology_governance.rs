//! Multi accelerator topology governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const TOPOLOGY: &str = include_str!("../../../docs/optimization/MULTI_ACCELERATOR_TOPOLOGY_CAPABILITY_MATRIX.toml");
const COLLECTIVE: &str = include_str!("../../../docs/optimization/PEER_TRANSFER_COLLECTIVE_POLICY.toml");
const SHARDING: &str = include_str!("../../../docs/optimization/MULTI_GPU_SHARDING_AGGREGATION_PLAN.toml");
const FALLBACK: &str = include_str!("../../../docs/optimization/MULTI_ACCELERATOR_FAILURE_FALLBACK_POLICY.toml");
const COVERAGE: &str = include_str!("../../../docs/optimization/END_TO_END_MULTI_ACCELERATOR_TOPOLOGY_TRANCHE_COVERAGE.toml");

#[test]
fn multi_accelerator_topology_sources_are_registered() {
    for key in [
        "CUDA_PEER_DEVICE_MEMORY",
        "CUDA_MULTI_DEVICE_P2P",
        "CUDA_DEVICE_P2P_ATTRIBUTES",
        "NCCL_COLLECTIVES",
        "NVML_NVLINK_METHODS",
        "VULKAN_DEVICE_GROUPS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn topology_capability_matrix_records_device_pair_link_peer_copy_atomic_fallback_and_diagnostics() {
    for required in [
        "topology_id",
        "accelerator_surface",
        "device_pair_policy",
        "link_class_policy",
        "peer_access_policy",
        "peer_copy_policy",
        "atomic_policy",
        "fallback_policy",
        "cuda-peer-access-matrix",
        "nvlink-health-and-bandwidth-class",
        "vulkan-device-group-topology",
    ] {
        assert!(TOPOLOGY.contains(required), "multi accelerator topology matrix must include {required}");
    }
}

#[test]
fn peer_transfer_collective_policy_records_rank_mapping_transfer_collective_order_hang_prevention_and_equivalence() {
    for required in [
        "collective_id",
        "data_surface",
        "rank_mapping_policy",
        "transfer_policy",
        "collective_policy",
        "ordering_policy",
        "hang_prevention_policy",
        "equivalence_policy",
        "scan-shard-output-allgather",
        "distributed-rule-database-broadcast",
        "frontier-counter-reduce-scatter",
    ] {
        assert!(COLLECTIVE.contains(required), "peer transfer collective policy must include {required}");
    }
}

#[test]
fn multi_gpu_sharding_plan_records_partition_halo_load_balance_aggregation_parity_and_scheduler_boundaries() {
    for required in [
        "shard_id",
        "workload_surface",
        "partition_policy",
        "halo_policy",
        "load_balance_policy",
        "aggregation_policy",
        "parity_policy",
        "scheduler_boundary_policy",
        "regex-haystack-byte-range-shards",
        "graph-frontier-device-shards",
        "pattern-database-replicated-shards",
    ] {
        assert!(SHARDING.contains(required), "multi GPU sharding plan must include {required}");
    }
}

#[test]
fn failure_fallback_policy_records_detection_containment_fallback_output_integrity_operator_privacy_and_links() {
    for required in [
        "failure_id",
        "failure_surface",
        "detection_policy",
        "containment_policy",
        "fallback_route_policy",
        "output_integrity_policy",
        "operator_effect_policy",
        "privacy_boundary",
        "peer-access-missing-or-revoked",
        "collective-rank-mismatch-or-timeout",
        "accelerator-link-health-degradation",
    ] {
        assert!(FALLBACK.contains(required), "multi accelerator failure fallback policy must include {required}");
    }
}

#[test]
fn plan_contains_multi_accelerator_topology_rows() {
    for row in [
        "VX-1481",
        "VX-1482",
        "VX-1483",
        "VX-1484",
        "VX-1485",
        "VX-1486",
        "VX-1487",
        "VX-1488",
        "VX-1489",
        "VX-1490",
        "VX-1491",
        "VX-1492",
        "VX-1493",
        "VX-1494",
        "VX-1495",
        "VX-1496",
        "VX-1497",
        "VX-1498",
        "VX-1499",
        "VX-1500",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn multi_accelerator_coverage_reuses_placement_capacity_capability_stratification_partition_memory_visibility_output_cache_backpressure_watchdog_profile_statistics_release_and_publication_authorities() {
    for required in [
        "VX-1481..VX-1500",
        "multi_accelerator_topology_capability_matrix",
        "peer_transfer_collective_policy",
        "multi_gpu_sharding_aggregation_plan",
        "multi_accelerator_failure_fallback_policy",
        "gpu_device_capacity_placement_policy",
        "scheduling_placement_topology_policy",
        "backend_capability_digests",
        "hardware_workload_stratification_matrix",
        "regex_cpu_gpu_partition",
        "memory_residency_tier_contracts",
        "transfer_overlap_pipeline_evidence",
        "host_device_visibility_policy",
        "output_slab_provenance",
        "cache_rebuild_invalidation_policy",
        "backpressure_queue_quota_policy",
        "runtime_watchdog_proofs",
        "profile_trace_metric_correlation_policy",
        "statistical_regression_gates",
        "release_health_feedback_loop",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "multi accelerator coverage must include {required}");
    }
}

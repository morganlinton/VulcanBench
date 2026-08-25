//! Host locality affinity governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const CONTRACTS: &str = include_str!("../../../docs/optimization/HOST_LOCALITY_AFFINITY_CONTRACTS.toml");
const MATRIX: &str = include_str!("../../../docs/optimization/CPU_GPU_TOPOLOGY_AFFINITY_MATRIX.toml");
const NUMA: &str = include_str!("../../../docs/optimization/NUMA_MEMORY_PLACEMENT_POLICY.toml");
const EVIDENCE: &str = include_str!("../../../docs/optimization/LOCALITY_REGRESSION_EVIDENCE_POLICY.toml");
const COVERAGE: &str = include_str!("../../../docs/optimization/END_TO_END_HOST_LOCALITY_AFFINITY_TRANCHE_COVERAGE.toml");

#[test]
fn host_locality_affinity_sources_are_registered() {
    for key in [
        "LINUX_KERNEL_NUMA_MEMORY_POLICY",
        "LINUX_KERNEL_CPUSETS",
        "LINUX_SCHED_AFFINITY",
        "KUBERNETES_CPU_MANAGER",
        "KUBERNETES_TOPOLOGY_MANAGER",
        "NVML_CPU_MEMORY_AFFINITY",
        "HWLOC_TOPOLOGY_API",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn host_locality_contracts_record_cpu_memory_device_cpuset_migration_fallback_and_authority_links() {
    for required in [
        "contract_id",
        "host_surface",
        "cpu_affinity_policy",
        "memory_policy",
        "device_locality_policy",
        "cpuset_policy",
        "migration_policy",
        "fallback_policy",
        "driver-submission-thread-locality",
        "pinned-staging-worker-locality",
        "io-parser-scan-thread-locality",
    ] {
        assert!(CONTRACTS.contains(required), "host locality contracts must include {required}");
    }
}

#[test]
fn cpu_gpu_topology_matrix_records_cpu_memory_io_kubernetes_sharing_privacy_and_links() {
    for required in [
        "affinity_id",
        "accelerator_surface",
        "cpu_set_policy",
        "memory_node_policy",
        "io_topology_policy",
        "kubernetes_alignment_policy",
        "sharing_policy",
        "privacy_boundary",
        "nvidia-gpu-local-cpu-mask",
        "storage-device-ingress-locality",
        "cpu-reference-route-locality",
    ] {
        assert!(MATRIX.contains(required), "CPU/GPU topology affinity matrix must include {required}");
    }
}

#[test]
fn numa_memory_placement_policy_records_policy_mode_node_selection_first_touch_migration_claim_scope_and_failure() {
    for required in [
        "placement_id",
        "allocation_surface",
        "policy_mode",
        "node_selection_policy",
        "first_touch_policy",
        "migration_policy",
        "claim_scope_policy",
        "failure_policy",
        "pinned-staging-numa-placement",
        "mmap-corpus-numa-placement",
        "operator-container-numa-alignment",
    ] {
        assert!(NUMA.contains(required), "NUMA memory placement policy must include {required}");
    }
}

#[test]
fn locality_regression_evidence_records_baseline_candidate_metrics_counters_correctness_and_decisions() {
    for required in [
        "evidence_id",
        "comparison_surface",
        "baseline_policy",
        "candidate_policy",
        "metric_policy",
        "counter_policy",
        "correctness_policy",
        "decision_policy",
        "driver-submission-affinity-regression",
        "pinned-staging-numa-regression",
        "cpu-reference-locality-regression",
    ] {
        assert!(EVIDENCE.contains(required), "locality regression evidence policy must include {required}");
    }
}

#[test]
fn plan_contains_host_locality_affinity_rows() {
    for row in [
        "VX-1501",
        "VX-1502",
        "VX-1503",
        "VX-1504",
        "VX-1505",
        "VX-1506",
        "VX-1507",
        "VX-1508",
        "VX-1509",
        "VX-1510",
        "VX-1511",
        "VX-1512",
        "VX-1513",
        "VX-1514",
        "VX-1515",
        "VX-1516",
        "VX-1517",
        "VX-1518",
        "VX-1519",
        "VX-1520",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn host_locality_coverage_reuses_placement_gpu_topology_memory_transfer_corpus_partition_stratification_budget_profile_statistics_output_and_publication_authorities() {
    for required in [
        "VX-1501..VX-1520",
        "host_locality_affinity_contracts",
        "cpu_gpu_topology_affinity_matrix",
        "numa_memory_placement_policy",
        "locality_regression_evidence_policy",
        "scheduling_placement_topology_policy",
        "gpu_device_capacity_placement_policy",
        "multi_accelerator_topology_capability_matrix",
        "memory_residency_tier_contracts",
        "pinned_staging_transfer_policy",
        "transfer_overlap_pipeline_evidence",
        "memory_pressure_oversubscription_response",
        "corpus_paging_planner",
        "gpu_direct_corpus_paging_capabilities",
        "regex_cpu_gpu_partition",
        "hardware_workload_stratification_matrix",
        "resource_rightsizing_capacity_policy",
        "resource_budget_policy",
        "backpressure_queue_quota_policy",
        "profile_trace_metric_correlation_policy",
        "statistical_regression_gates",
        "output_slab_provenance",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "host locality coverage must include {required}");
    }
}

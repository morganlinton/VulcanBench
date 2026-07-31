//! Capacity autoscaling governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const AUTOSCALE: &str = include_str!("../../../docs/optimization/WORKLOAD_AUTOSCALING_POLICY.toml");
const CAPACITY: &str =
    include_str!("../../../docs/optimization/RESOURCE_RIGHTSIZING_CAPACITY_POLICY.toml");
const PLACEMENT: &str =
    include_str!("../../../docs/optimization/SCHEDULING_PLACEMENT_TOPOLOGY_POLICY.toml");
const GPU: &str = include_str!("../../../docs/optimization/GPU_DEVICE_CAPACITY_PLACEMENT_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_CAPACITY_AUTOSCALING_TRANCHE_COVERAGE.toml");

#[test]
fn capacity_autoscaling_sources_are_registered() {
    for key in [
        "KUBERNETES_HPA",
        "KEDA_SCALING_DEPLOYMENTS",
        "KUBERNETES_RESOURCE_MANAGEMENT",
        "KUBERNETES_NODE_AFFINITY",
        "KUBERNETES_TOPOLOGY_SPREAD",
        "KUBERNETES_TAINTS_TOLERATIONS",
        "KUBERNETES_PRIORITY_PREEMPTION",
        "KUBERNETES_DEVICE_PLUGINS",
        "NVIDIA_K8S_DEVICE_PLUGIN",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn workload_autoscaling_policy_records_signals_bounds_behavior_freshness_pause_rollout_and_gate_effects() {
    for required in [
        "autoscale_id",
        "workload_surface",
        "scale_signal_policy",
        "replica_bounds_policy",
        "scale_behavior_policy",
        "metric_freshness_policy",
        "pause_policy",
        "rollout_interaction_policy",
        "release_gate_effect",
        "operator-api-hpa",
        "event-driven-worker-keda",
    ] {
        assert!(
            AUTOSCALE.contains(required),
            "workload autoscaling policy must include {required}"
        );
    }
}

#[test]
fn resource_rightsizing_policy_records_requests_limits_usage_rightsizing_quotas_saturation_cost_capacity_and_gates() {
    for required in [
        "capacity_id",
        "workload_surface",
        "request_limit_policy",
        "observed_usage_policy",
        "rightsizing_policy",
        "quota_link_policy",
        "saturation_signal_policy",
        "cost_capacity_policy",
        "release_gate_effect",
        "operator-container-rightsizing",
        "release-workload-capacity-envelope",
    ] {
        assert!(
            CAPACITY.contains(required),
            "resource rightsizing capacity policy must include {required}"
        );
    }
}

#[test]
fn scheduling_placement_policy_records_node_assignment_topology_taints_priority_rollout_privacy_and_gates() {
    for required in [
        "placement_id",
        "workload_surface",
        "node_assignment_policy",
        "topology_spread_policy",
        "taint_toleration_policy",
        "priority_preemption_policy",
        "rollout_policy",
        "privacy_boundary",
        "release_gate_effect",
        "operator-zone-spread-placement",
        "batch-worker-priority-placement",
    ] {
        assert!(
            PLACEMENT.contains(required),
            "scheduling placement topology policy must include {required}"
        );
    }
}

#[test]
fn gpu_device_capacity_policy_records_plugin_resources_placement_sharing_runtime_capacity_and_gate_effects() {
    for required in [
        "gpu_id",
        "device_surface",
        "device_plugin_policy",
        "resource_request_policy",
        "placement_policy",
        "sharing_partition_policy",
        "runtime_config_policy",
        "capacity_evidence_policy",
        "release_gate_effect",
        "nvidia-gpu-operator-worker-placement",
        "mixed-cpu-gpu-routing-capacity",
    ] {
        assert!(
            GPU.contains(required),
            "gpu device capacity placement policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_capacity_autoscaling_rows() {
    for row in [
        "VX-1321",
        "VX-1322",
        "VX-1323",
        "VX-1324",
        "VX-1325",
        "VX-1326",
        "VX-1327",
        "VX-1328",
        "VX-1329",
        "VX-1330",
        "VX-1331",
        "VX-1332",
        "VX-1333",
        "VX-1334",
        "VX-1335",
        "VX-1336",
        "VX-1337",
        "VX-1338",
        "VX-1339",
        "VX-1340",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn capacity_autoscaling_coverage_reuses_resource_deployment_readiness_failure_publication_and_dedup_authorities() {
    for required in [
        "VX-1321..VX-1340",
        "workload_autoscaling_policy",
        "resource_rightsizing_capacity_policy",
        "scheduling_placement_topology_policy",
        "gpu_device_capacity_placement_policy",
        "resource_dos_governance",
        "operator_deployment_coverage",
        "operational_readiness_coverage",
        "failure_injection_resilience_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "capacity autoscaling tranche coverage must include {required}"
        );
    }
}

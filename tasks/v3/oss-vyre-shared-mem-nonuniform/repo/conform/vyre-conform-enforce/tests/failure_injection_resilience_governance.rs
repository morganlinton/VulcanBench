//! Failure injection resilience governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SCOPE: &str = include_str!("../../../docs/optimization/CHAOS_EXPERIMENT_SCOPE_POLICY.toml");
const DISRUPTION: &str =
    include_str!("../../../docs/optimization/POD_NODE_DISRUPTION_RESILIENCE_POLICY.toml");
const NETWORK_IO: &str =
    include_str!("../../../docs/optimization/NETWORK_IO_FAULT_INJECTION_POLICY.toml");
const STRESS: &str =
    include_str!("../../../docs/optimization/STRESS_RESOURCE_SATURATION_EXPERIMENT_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_FAILURE_INJECTION_RESILIENCE_TRANCHE_COVERAGE.toml");

#[test]
fn failure_injection_resilience_sources_are_registered() {
    for key in [
        "CHAOS_MESH_PODCHAOS",
        "CHAOS_MESH_NETWORKCHAOS",
        "CHAOS_MESH_IOCHAOS",
        "CHAOS_MESH_STRESSCHAOS",
        "KUBERNETES_DISRUPTIONS",
        "KUBERNETES_PDB",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn chaos_scope_policy_records_target_blast_radius_preconditions_aborts_observation_owner_and_publication_boundary() {
    for required in [
        "experiment_id",
        "fault_family",
        "target_selector_policy",
        "blast_radius_policy",
        "precondition_policy",
        "abort_policy",
        "observation_policy",
        "owner_route",
        "publication_boundary",
        "operator-single-surface-chaos-scope",
        "release-promotion-resilience-scope",
    ] {
        assert!(
            SCOPE.contains(required),
            "chaos experiment scope policy must include {required}"
        );
    }
}

#[test]
fn pod_node_disruption_policy_records_fault_actions_targets_budgets_probes_recovery_runbooks_gate_effects_and_evidence() {
    for required in [
        "disruption_id",
        "fault_action_policy",
        "target_policy",
        "availability_budget_policy",
        "probe_policy",
        "recovery_policy",
        "runbook_policy",
        "release_gate_effect",
        "evidence_policy",
        "pod-failure-readiness-resilience",
        "container-kill-driver-worker-resilience",
    ] {
        assert!(
            DISRUPTION.contains(required),
            "pod node disruption resilience policy must include {required}"
        );
    }
}

#[test]
fn network_io_fault_policy_records_injection_selectors_expected_behavior_timeouts_integrity_observation_and_gate_effects() {
    for required in [
        "fault_id",
        "fault_surface",
        "injection_policy",
        "selector_policy",
        "expected_behavior_policy",
        "timeout_retry_policy",
        "data_integrity_policy",
        "observation_policy",
        "release_gate_effect",
        "network-delay-loss-dependency-resilience",
        "file-io-latency-fault-state-resilience",
    ] {
        assert!(
            NETWORK_IO.contains(required),
            "network io fault injection policy must include {required}"
        );
    }
}

#[test]
fn stress_policy_records_surfaces_injection_resource_budgets_backpressure_shedding_fairness_observation_and_gate_effects() {
    for required in [
        "stress_id",
        "stress_surface",
        "injection_policy",
        "resource_budget_policy",
        "backpressure_policy",
        "load_shedding_policy",
        "fairness_policy",
        "observation_policy",
        "release_gate_effect",
        "cpu-memory-pressure-operator-resilience",
        "gpu-runtime-pressure-resilience",
    ] {
        assert!(
            STRESS.contains(required),
            "stress resource saturation experiment policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_failure_injection_resilience_rows() {
    for row in [
        "VX-1301",
        "VX-1302",
        "VX-1303",
        "VX-1304",
        "VX-1305",
        "VX-1306",
        "VX-1307",
        "VX-1308",
        "VX-1309",
        "VX-1310",
        "VX-1311",
        "VX-1312",
        "VX-1313",
        "VX-1314",
        "VX-1315",
        "VX-1316",
        "VX-1317",
        "VX-1318",
        "VX-1319",
        "VX-1320",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn failure_injection_coverage_reuses_readiness_recovery_rollout_resource_publication_and_dedup_authorities() {
    for required in [
        "VX-1301..VX-1320",
        "chaos_experiment_scope_policy",
        "pod_node_disruption_resilience_policy",
        "network_io_fault_injection_policy",
        "stress_resource_saturation_experiment_policy",
        "operational_readiness_coverage",
        "operator_state_recovery_coverage",
        "staged_rollout_coverage",
        "resource_dos_governance",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "failure injection resilience tranche coverage must include {required}"
        );
    }
}

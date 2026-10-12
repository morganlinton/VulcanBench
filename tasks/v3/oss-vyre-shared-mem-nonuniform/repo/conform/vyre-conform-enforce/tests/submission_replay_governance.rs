//! Submission replay governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const TOPOLOGY: &str = include_str!("../../../docs/optimization/COMMAND_TOPOLOGY_STABILITY_CONTRACTS.toml");
const CAPABILITY: &str = include_str!("../../../docs/optimization/BACKEND_COMMAND_REPLAY_CAPABILITY_MATRIX.toml");
const DECISION: &str = include_str!("../../../docs/optimization/SUBMISSION_REPLAY_DECISION_POLICY.toml");
const EVIDENCE: &str = include_str!("../../../docs/optimization/SUBMISSION_REPLAY_EVIDENCE_POLICY.toml");
const COVERAGE: &str = include_str!("../../../docs/optimization/END_TO_END_SUBMISSION_REPLAY_TRANCHE_COVERAGE.toml");

#[test]
fn submission_replay_sources_are_registered() {
    for key in [
        "CUDA_GRAPHS",
        "WGPU_COMMANDS",
        "VULKAN_COMMAND_BUFFERS",
        "WEBGPU_COMMAND_BUFFERS",
        "METAL_INDIRECT_COMMAND_BUFFERS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn command_topology_stability_contracts_record_shape_resource_update_invalidation_sync_and_equivalence() {
    for required in [
        "topology_id",
        "backend_surfaces",
        "command_shape_policy",
        "resource_identity_policy",
        "parameter_update_policy",
        "invalidation_policy",
        "synchronization_dependency_policy",
        "output_equivalence_policy",
        "authority_links",
        "scan-dispatch-stable-topology",
        "multi-pass-output-readback-topology",
        "resident-hot-shape-command-topology",
    ] {
        assert!(TOPOLOGY.contains(required), "command topology contract must include {required}");
    }
}

#[test]
fn backend_command_replay_capability_matrix_records_native_reuse_update_pending_dynamic_and_probe_policies() {
    for required in [
        "backend_id",
        "replay_surface",
        "reusable_object_policy",
        "update_policy",
        "pending_or_capture_policy",
        "dynamic_parameter_policy",
        "unsupported_case_policy",
        "capability_probe_policy",
        "cuda-graph-replay",
        "vulkan-command-buffer-replay",
        "webgpu-wgpu-command-template",
        "metal-indirect-command-buffer-replay",
    ] {
        assert!(CAPABILITY.contains(required), "backend command replay capability matrix must include {required}");
    }
}

#[test]
fn submission_replay_decision_policy_records_inputs_amortization_boundaries_adoption_rejection_and_telemetry() {
    for required in [
        "decision_id",
        "candidate_surface",
        "input_signal_policy",
        "amortization_policy",
        "topology_policy",
        "resource_policy",
        "scheduler_boundary_policy",
        "synchronization_boundary_policy",
        "adoption_policy",
        "rejection_policy",
        "telemetry_policy",
        "hot-stable-shape-record-and-replay",
        "shape-drift-recapture-or-reencode",
        "resident-persistent-kernel-wins",
    ] {
        assert!(DECISION.contains(required), "submission replay decision policy must include {required}");
    }
}

#[test]
fn submission_replay_evidence_policy_records_costs_correctness_invalidation_privacy_and_backend_records() {
    for required in [
        "evidence_id",
        "backend_surface",
        "topology_digest",
        "repeat_count_policy",
        "capture_or_record_cost_policy",
        "replay_cost_policy",
        "update_or_reencode_policy",
        "correctness_equivalence_policy",
        "invalidation_reason_policy",
        "privacy_boundary",
        "cuda-graph-update-replay-record",
        "webgpu-reencode-template-record",
        "vulkan-metal-reusable-command-record",
    ] {
        assert!(EVIDENCE.contains(required), "submission replay evidence policy must include {required}");
    }
}

#[test]
fn plan_contains_submission_replay_rows() {
    for row in [
        "VX-1441",
        "VX-1442",
        "VX-1443",
        "VX-1444",
        "VX-1445",
        "VX-1446",
        "VX-1447",
        "VX-1448",
        "VX-1449",
        "VX-1450",
        "VX-1451",
        "VX-1452",
        "VX-1453",
        "VX-1454",
        "VX-1455",
        "VX-1456",
        "VX-1457",
        "VX-1458",
        "VX-1459",
        "VX-1460",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn submission_replay_coverage_reuses_backend_native_sync_cache_profile_stats_output_runtime_and_publication_authorities() {
    for required in [
        "VX-1441..VX-1460",
        "command_topology_stability_contracts",
        "backend_command_replay_capability_matrix",
        "submission_replay_decision_policy",
        "submission_replay_evidence_policy",
        "cuda_graph_update_evidence",
        "wgpu_command_reuse_classifier",
        "host_device_visibility_policy",
        "memory_ordering_atomic_contracts",
        "backend_handle_lifetime_provenance",
        "cache_rebuild_invalidation_policy",
        "profile_trace_metric_correlation_policy",
        "statistical_regression_gates",
        "output_slab_provenance",
        "runtime_scheduler_boundary",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "submission replay coverage must include {required}");
    }
}

//! Continuous profiling governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SIGNALS: &str =
    include_str!("../../../docs/optimization/CONTINUOUS_PROFILING_SIGNAL_CONTRACTS.toml");
const COLLECTION: &str =
    include_str!("../../../docs/optimization/PROFILING_COLLECTION_SECURITY_POLICY.toml");
const CORRELATION: &str =
    include_str!("../../../docs/optimization/PROFILE_TRACE_METRIC_CORRELATION_POLICY.toml");
const TRIAGE: &str =
    include_str!("../../../docs/optimization/PERFORMANCE_REGRESSION_PROFILE_TRIAGE_POLICY.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_CONTINUOUS_PROFILING_TRANCHE_COVERAGE.toml"
);

#[test]
fn continuous_profiling_sources_are_registered() {
    for key in [
        "OPENTELEMETRY_PROFILES_SPEC",
        "OPENTELEMETRY_PROFILE_SIGNAL",
        "OPENTELEMETRY_PROFILE_MAPPINGS",
        "OPENTELEMETRY_PPROF_COMPATIBILITY",
        "GOOGLE_PPROF",
        "LINUX_PERF_SECURITY",
        "GRAFANA_PYROSCOPE",
        "PYROSCOPE_CLIENT_COLLECTION",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn profiling_signal_contract_records_sample_mapping_symbolization_privacy_and_gate_fields() {
    for required in [
        "profile_id",
        "profile_type",
        "sample_event_policy",
        "stack_context_policy",
        "resource_attribute_policy",
        "mapping_identity_policy",
        "symbolization_policy",
        "pprof_compatibility_policy",
        "correlation_policy",
        "privacy_boundary",
        "retention_policy",
        "release_gate_effect",
        "operator-runtime-oncpu-profile",
        "bench-harness-stage-profile",
        "allocator-heap-pressure-profile",
    ] {
        assert!(SIGNALS.contains(required), "profiling signal contract must include {required}");
    }
}

#[test]
fn profiling_collection_security_records_privileges_sampling_overhead_storage_redaction_and_failure_policy() {
    for required in [
        "collector_id",
        "collection_mode",
        "target_surface",
        "privilege_policy",
        "sampling_policy",
        "overhead_budget_policy",
        "cardinality_policy",
        "storage_policy",
        "network_policy",
        "redaction_policy",
        "failure_policy",
        "authority_links",
        "linux-perf-operator-runtime",
        "pyroscope-alloy-ebpf-profile-collector",
        "bench-harness-profile-capture",
    ] {
        assert!(COLLECTION.contains(required), "profiling collection policy must include {required}");
    }
}

#[test]
fn profile_correlation_records_trace_metric_build_identity_pprof_provenance_and_regression_links() {
    for required in [
        "correlation_id",
        "profile_id",
        "trace_id_policy",
        "metric_policy",
        "profile_mapping_policy",
        "build_identity_policy",
        "time_window_policy",
        "pprof_policy",
        "provenance_policy",
        "regression_policy",
        "redaction_policy",
        "TRACE_CONTEXT_TELEMETRY_CONTRACTS.toml",
        "METRICS_EXPOSITION_CONTRACTS.toml",
        "CROSS_SIGNAL_AUDIT_CORRELATION.toml",
    ] {
        assert!(CORRELATION.contains(required), "profile correlation policy must include {required}");
    }
}

#[test]
fn regression_profile_triage_records_baseline_candidate_diff_attribution_rollout_and_gate_effects() {
    for required in [
        "triage_id",
        "trigger_policy",
        "baseline_policy",
        "candidate_policy",
        "profile_diff_policy",
        "hot_path_policy",
        "attribution_policy",
        "operator_action_policy",
        "rollout_policy",
        "privacy_boundary",
        "release_gate_effect",
        "scan-latency-regression-profile-triage",
        "gpu-throughput-profile-triage",
        "allocation-regression-profile-triage",
    ] {
        assert!(TRIAGE.contains(required), "profile triage policy must include {required}");
    }
}

#[test]
fn plan_contains_continuous_profiling_rows() {
    for row in [
        "VX-1361",
        "VX-1362",
        "VX-1363",
        "VX-1364",
        "VX-1365",
        "VX-1366",
        "VX-1367",
        "VX-1368",
        "VX-1369",
        "VX-1370",
        "VX-1371",
        "VX-1372",
        "VX-1373",
        "VX-1374",
        "VX-1375",
        "VX-1376",
        "VX-1377",
        "VX-1378",
        "VX-1379",
        "VX-1380",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn continuous_profiling_coverage_reuses_existing_signal_readiness_capacity_rollout_and_publication_authorities() {
    for required in [
        "VX-1361..VX-1380",
        "continuous_profiling_signal_contracts",
        "profiling_collection_security_policy",
        "profile_trace_metric_correlation_policy",
        "performance_regression_profile_triage_policy",
        "trace_context_telemetry_contracts",
        "metrics_exposition_contracts",
        "cross_signal_audit_correlation",
        "operational_readiness_coverage",
        "capacity_autoscaling_coverage",
        "staged_rollout_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "continuous profiling coverage must include {required}");
    }
}

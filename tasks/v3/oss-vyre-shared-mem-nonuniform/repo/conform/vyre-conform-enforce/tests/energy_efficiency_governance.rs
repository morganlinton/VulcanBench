//! Energy efficiency governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const TELEMETRY: &str =
    include_str!("../../../docs/optimization/POWER_THERMAL_TELEMETRY_CONTRACTS.toml");
const REGRESSION: &str =
    include_str!("../../../docs/optimization/ENERGY_EFFICIENCY_REGRESSION_POLICY.toml");
const ACCOUNTING: &str =
    include_str!("../../../docs/optimization/CARBON_COST_EFFICIENCY_ACCOUNTING_POLICY.toml");
const RESPONSE: &str =
    include_str!("../../../docs/optimization/POWER_CAP_THROTTLE_RESPONSE_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_ENERGY_EFFICIENCY_TRANCHE_COVERAGE.toml");

#[test]
fn energy_efficiency_sources_are_registered() {
    for key in [
        "NVIDIA_DCGM_POWER_PROFILES",
        "NVIDIA_NVML_DEVICE_POWER",
        "NVIDIA_DCGM_PROFILING_GROUPS",
        "LINUX_POWERCAP_RAPL",
        "GREEN_SOFTWARE_SCI",
        "KEPLER_ENERGY_EXPORTER",
        "OPENTELEMETRY_HARDWARE_SEMCONV",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn power_thermal_telemetry_records_sensors_units_windows_attribution_throttle_thermal_and_exports() {
    for required in [
        "telemetry_id",
        "measurement_surface",
        "sensor_source_policy",
        "unit_policy",
        "sampling_window_policy",
        "attribution_policy",
        "clock_throttle_policy",
        "thermal_policy",
        "profile_group_policy",
        "metric_export_policy",
        "privacy_boundary",
        "gpu-power-thermal-telemetry",
        "cpu-rapl-energy-telemetry",
        "container-pod-energy-attribution",
    ] {
        assert!(TELEMETRY.contains(required), "power telemetry contract must include {required}");
    }
}

#[test]
fn energy_efficiency_regression_records_functional_units_baselines_energy_metrics_throttle_stats_and_gate_effects() {
    for required in [
        "efficiency_gate_id",
        "workload_surface",
        "functional_unit_policy",
        "baseline_policy",
        "candidate_policy",
        "energy_metric_policy",
        "performance_metric_policy",
        "thermal_throttle_policy",
        "statistical_policy",
        "decision_policy",
        "release_gate_effect",
        "gpu-throughput-per-watt-regression",
        "cpu-reference-joules-per-scan-regression",
        "operator-load-energy-regression",
    ] {
        assert!(REGRESSION.contains(required), "energy regression policy must include {required}");
    }
}

#[test]
fn carbon_cost_accounting_records_boundary_functional_unit_energy_carbon_embodied_cost_claim_and_reporting_policies() {
    for required in [
        "accounting_id",
        "boundary_policy",
        "functional_unit_policy",
        "energy_policy",
        "carbon_intensity_policy",
        "embodied_emissions_policy",
        "cost_capacity_policy",
        "claim_scope_policy",
        "reporting_policy",
        "privacy_boundary",
        "release_gate_effect",
        "scan-energy-carbon-rate-accounting",
        "operator-release-energy-cost-accounting",
        "benchmark-perf-per-cost-class-accounting",
    ] {
        assert!(ACCOUNTING.contains(required), "carbon cost accounting policy must include {required}");
    }
}

#[test]
fn power_cap_throttle_response_records_triggers_classification_actions_profiles_profiler_contention_rollout_and_runbook_links() {
    for required in [
        "response_id",
        "trigger_policy",
        "classification_policy",
        "operator_action_policy",
        "power_profile_policy",
        "profiler_contention_policy",
        "rollout_policy",
        "runbook_policy",
        "evidence_policy",
        "privacy_boundary",
        "release_gate_effect",
        "gpu-power-limit-throttle-response",
        "cpu-powercap-constraint-response",
        "collector-efficiency-overhead-response",
    ] {
        assert!(RESPONSE.contains(required), "power cap response policy must include {required}");
    }
}

#[test]
fn plan_contains_energy_efficiency_rows() {
    for row in [
        "VX-1401",
        "VX-1402",
        "VX-1403",
        "VX-1404",
        "VX-1405",
        "VX-1406",
        "VX-1407",
        "VX-1408",
        "VX-1409",
        "VX-1410",
        "VX-1411",
        "VX-1412",
        "VX-1413",
        "VX-1414",
        "VX-1415",
        "VX-1416",
        "VX-1417",
        "VX-1418",
        "VX-1419",
        "VX-1420",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn energy_efficiency_coverage_reuses_metrics_profiling_capacity_experiment_rollout_slo_runbook_and_publication_authorities() {
    for required in [
        "VX-1401..VX-1420",
        "power_thermal_telemetry_contracts",
        "energy_efficiency_regression_policy",
        "carbon_cost_efficiency_accounting_policy",
        "power_cap_throttle_response_policy",
        "metrics_exposition_contracts",
        "continuous_profiling_coverage",
        "capacity_autoscaling_coverage",
        "optimization_experiment_design_coverage",
        "operational_readiness_coverage",
        "staged_rollout_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "energy efficiency coverage must include {required}");
    }
}

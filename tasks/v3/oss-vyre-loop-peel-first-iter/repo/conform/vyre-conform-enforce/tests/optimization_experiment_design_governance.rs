//! Optimization experiment design governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const DESIGN: &str =
    include_str!("../../../docs/optimization/OPTIMIZATION_EXPERIMENT_DESIGN_POLICY.toml");
const STRATA: &str =
    include_str!("../../../docs/optimization/HARDWARE_WORKLOAD_STRATIFICATION_MATRIX.toml");
const ABLATION: &str =
    include_str!("../../../docs/optimization/ABLATION_AND_COUNTERFACTUAL_EVIDENCE_POLICY.toml");
const MULTIPLE: &str =
    include_str!("../../../docs/optimization/MULTIPLE_COMPARISON_DECISION_POLICY.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_OPTIMIZATION_EXPERIMENT_DESIGN_TRANCHE_COVERAGE.toml"
);

#[test]
fn optimization_experiment_design_sources_are_registered() {
    for key in [
        "NIST_DOE_DESIGN_SELECTION",
        "NIST_DOE_OBJECTIVES",
        "NIST_DOE_FACTOR_SELECTION",
        "NIST_RANDOMIZED_BLOCK_DESIGN",
        "NIST_FULL_FACTORIAL_DESIGN",
        "NIST_MULTIPLE_COMPARISONS",
        "CRITERION_ANALYSIS_PROCESS",
        "GOOGLE_BENCHMARK_COMPARE_TOOLS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn experiment_design_policy_records_hypotheses_factors_blocks_controls_and_run_matrix() {
    for required in [
        "experiment_id",
        "objective_class",
        "hypothesis_policy",
        "primary_factor_policy",
        "nuisance_factor_policy",
        "response_policy",
        "design_selection_policy",
        "factor_level_policy",
        "blocking_policy",
        "randomization_policy",
        "control_policy",
        "run_matrix_policy",
        "decision_sink",
        "regex-route-selection-experiment",
        "gpu-kernel-tuning-factorial-experiment",
        "allocation-cache-policy-ablation-experiment",
    ] {
        assert!(DESIGN.contains(required), "experiment design policy must include {required}");
    }
}

#[test]
fn stratification_matrix_records_block_workload_hardware_capacity_replication_and_generalization_policy() {
    for required in [
        "stratum_id",
        "stratification_surface",
        "block_factor_policy",
        "workload_factor_policy",
        "hardware_factor_policy",
        "capacity_factor_policy",
        "replication_policy",
        "generalization_policy",
        "privacy_boundary",
        "authority_links",
        "cpu-regex-workload-blocks",
        "gpu-backend-workload-blocks",
        "operator-load-release-blocks",
    ] {
        assert!(STRATA.contains(required), "stratification matrix must include {required}");
    }
}

#[test]
fn ablation_policy_records_baseline_candidate_counterfactual_controls_interactions_evidence_and_decisions() {
    for required in [
        "ablation_id",
        "target_change",
        "baseline_policy",
        "candidate_policy",
        "ablation_policy",
        "counterfactual_policy",
        "control_policy",
        "interaction_policy",
        "evidence_policy",
        "decision_policy",
        "privacy_boundary",
        "regex-prefilter-route-ablation",
        "gpu-fusion-pass-ablation",
        "cache-invalidation-policy-ablation",
    ] {
        assert!(ABLATION.contains(required), "ablation policy must include {required}");
    }
}

#[test]
fn multiple_comparison_policy_records_family_scope_correction_alpha_effect_exploratory_winner_and_rollback_fields() {
    for required in [
        "comparison_family_id",
        "family_scope_policy",
        "primary_endpoint_policy",
        "comparison_set_policy",
        "correction_policy",
        "alpha_policy",
        "effect_size_policy",
        "exploratory_policy",
        "winner_policy",
        "rollback_policy",
        "authority_links",
        "regex-route-family-comparison",
        "gpu-tuning-factor-family-comparison",
        "operator-release-load-comparison",
    ] {
        assert!(MULTIPLE.contains(required), "multiple comparison policy must include {required}");
    }
}

#[test]
fn plan_contains_optimization_experiment_design_rows() {
    for row in [
        "VX-1381",
        "VX-1382",
        "VX-1383",
        "VX-1384",
        "VX-1385",
        "VX-1386",
        "VX-1387",
        "VX-1388",
        "VX-1389",
        "VX-1390",
        "VX-1391",
        "VX-1392",
        "VX-1393",
        "VX-1394",
        "VX-1395",
        "VX-1396",
        "VX-1397",
        "VX-1398",
        "VX-1399",
        "VX-1400",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn experiment_design_coverage_reuses_benchmark_statistics_profiling_capacity_rollout_and_publication_authorities() {
    for required in [
        "VX-1381..VX-1400",
        "optimization_experiment_design_policy",
        "hardware_workload_stratification_matrix",
        "ablation_and_counterfactual_evidence_policy",
        "multiple_comparison_decision_policy",
        "benchmark_methodology_contracts",
        "statistical_regression_gates",
        "continuous_profiling_coverage",
        "performance_validation_coverage",
        "capacity_autoscaling_coverage",
        "staged_rollout_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(COVERAGE.contains(required), "experiment design coverage must include {required}");
    }
}

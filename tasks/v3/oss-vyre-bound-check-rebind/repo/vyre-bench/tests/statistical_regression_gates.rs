//! Statistical regression gates test suite.

const GATES: &str =
    include_str!("../../docs/optimization/STATISTICAL_REGRESSION_GATES.toml");

#[test]
fn statistical_regression_gates_require_baselines_effect_sizes_confidence_and_decisions() {
    for required in [
        "gate_id",
        "benchmark_id",
        "baseline_digest",
        "candidate_digest",
        "effect_size",
        "confidence_level",
        "regression_threshold",
        "noise_floor",
        "decision",
        "route_change_allowed",
        "allow-route-change",
        "block-route-change",
    ] {
        assert!(
            GATES.contains(required),
            "statistical regression gate must include {required}"
        );
    }
}

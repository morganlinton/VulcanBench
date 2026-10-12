//! Metrics exposition contracts test suite.

const METRICS: &str = include_str!("../../docs/optimization/METRICS_EXPOSITION_CONTRACTS.toml");

#[test]
fn metrics_exposition_contracts_require_units_labels_cardinality_and_privacy() {
    for required in [
        "metric_name",
        "metric_type",
        "unit",
        "labels",
        "sample_value_contract",
        "counter_schema",
        "privacy_class",
        "cardinality_limit",
        "counter",
        "gauge",
        "histogram",
    ] {
        assert!(
            METRICS.contains(required),
            "metrics exposition contract must include {required}"
        );
    }
}

//! Benchmark methodology contracts test suite.

const METHODOLOGY: &str =
    include_str!("../../docs/optimization/BENCHMARK_METHODOLOGY_CONTRACTS.toml");

#[test]
fn benchmark_methodology_contracts_record_fixtures_sampling_counters_environment_and_noise() {
    for required in [
        "benchmark_id",
        "owning_crate",
        "fixture_digest",
        "warmup_policy",
        "sample_policy",
        "measurement_unit",
        "counter_set",
        "environment_digest",
        "noise_controls",
        "privacy_class",
    ] {
        assert!(
            METHODOLOGY.contains(required),
            "benchmark methodology contract must include {required}"
        );
    }
}

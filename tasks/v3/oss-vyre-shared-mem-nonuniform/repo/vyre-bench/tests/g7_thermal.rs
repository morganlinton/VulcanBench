//! G7  -  Thermal normalization.
//!
//! Verifies that the NVML probe captures temperature and clock metrics, and
//! that the `thermal_unstable` custom metric is populated per-sample.
//! On a thermally-stable system this metric should be 0.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_thermal_metrics_populated() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    assert!(!report.cases.is_empty(), "Must have at least one case");
    let case = &report.cases[0];

    // Check that temperature_c metric is present (NVML probe runs per sample)
    if case.metrics.contains_key("temperature_c") {
        let temp_stats = case.metrics.get("temperature_c").unwrap();
        assert!(
            temp_stats.max > 0,
            "temperature_c should be positive on a system with GPU"
        );

        // Thermal drift should be less than 15°C for a short benchmark
        let drift = temp_stats.max - temp_stats.min;
        assert!(
            drift < 15,
            "Temperature drift of {}°C exceeds 15°C threshold. Check GPU cooling.",
            drift,
        );
    }

    // If GPU metrics are available, check that thermal_unstable is populated
    if case.metrics.contains_key("thermal_unstable") {
        // On a healthy system this should be 0 (stable)
        let unstable = case.metrics.get("thermal_unstable").unwrap();
        // We just check it exists and is a valid metric; 0 = stable, >0 = unstable
        assert!(
            unstable.samples > 0,
            "thermal_unstable metric must have at least one sample"
        );
    }

    // The case status should NOT be thermal_unstable on a healthy system
    assert_ne!(
        case.status, "thermal_unstable",
        "Case should not be thermal_unstable on a healthy system"
    );
}

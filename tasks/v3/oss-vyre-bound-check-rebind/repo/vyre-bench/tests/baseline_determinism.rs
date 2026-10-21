//! Baseline determinism test.
#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_baseline_determinism() {
    let mut config = RunConfig::default();
    config.warmup_samples = 3;
    config.baseline_warmup_runs = 5;
    config.measured_samples = Some(30);
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Custom("deterministic_test"), &config);

    assert_eq!(report.cases.len(), 1, "Should run exactly 1 case");
    let case = &report.cases[0];

    let baseline_stats = case
        .metrics
        .get("baseline_wall_ns")
        .expect("baseline_wall_ns must be present");
    let cv = baseline_stats.stddev / baseline_stats.mean;

    assert!(
        cv < 0.05,
        "Baseline coefficient of variation {} >= 0.05. Baseline is not deterministic enough.",
        cv
    );
}

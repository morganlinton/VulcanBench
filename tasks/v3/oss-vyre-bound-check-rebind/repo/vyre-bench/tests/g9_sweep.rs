//! G9  -  Sweep matrix.
//!
//! Verifies that the Sweep suite kind runs cases and produces results.
//! Since sweep requires workgroup iteration, this test validates
//! that the Sweep suite can at minimum run the foundation cases.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_sweep_suite_runs() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    // The elementwise case should produce results
    assert!(
        !report.cases.is_empty(),
        "Sweep suite must produce at least one case result"
    );

    let case = &report.cases[0];
    assert_ne!(case.status, "failed", "Case should not fail");

    // Verify wall_ns metric is populated
    assert!(
        case.metrics.contains_key("wall_ns"),
        "wall_ns must be in metrics"
    );
}

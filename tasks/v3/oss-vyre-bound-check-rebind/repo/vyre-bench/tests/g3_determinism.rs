//! Determinism gate test.
#![allow(missing_docs, clippy::field_reassign_with_default, unsafe_code)]

use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_determinism_gate() {
    let mut config = RunConfig::default();
    config.measured_samples = Some(30);
    config.determinism_runs = 3;
    config.case_ids = vec!["synthetic.flaky".to_string()];

    let registry = vyre_bench::registry::collect_all();

    let report = execute_suite(&registry, SuiteKind::Custom("flaky_test"), &config);
    assert_eq!(report.cases.len(), 1);
    let case = &report.cases[0];

    assert_eq!(
        case.status, "unstable",
        "Flaky case should be marked unstable"
    );
    let stats = case.metrics.get("wall_ns").expect("Missing wall_ns");
    assert!(
        stats.determinism_cv.is_some(),
        "determinism_cv should be populated"
    );
    assert!(
        stats.determinism_cv.unwrap() > 0.05,
        "CV should be high for flaky case"
    );
}

#[test]
fn test_stable_determinism() {
    // 1M elements via the cpu-ref interpreter is ~3 minutes per
    // sample at the default 30. The contract under test is just
    // "CV is below 0.05 on a stable case"  -  five samples are plenty
    // to compute that, but the runner enforces a CLT-validity gate
    // (>= 30 samples) unless `VYRE_ALLOW_FEW_SAMPLES=1` is set.
    // Setting the env var keeps this test runnable from
    // `cargo test --workspace` while letting the production bench
    // path keep its statistical floor.
    // 1M elements via the cpu-ref interpreter is ~3 minutes per
    // sample at the default 30. The CV-below-0.05 contract holds at
    // five samples; the runner enforces a CLT-validity gate
    // (>= 30 samples) unless `VYRE_ALLOW_FEW_SAMPLES=1` is set.
    // SAFETY: cargo test does not parallelize tests across processes
    // and this env var is only read at run-config construction.
    unsafe {
        std::env::set_var("VYRE_ALLOW_FEW_SAMPLES", "1");
    }
    let mut config = RunConfig::default();
    config.measured_samples = Some(5);
    config.determinism_runs = 3;
    config.backend_id = Some("cpu-ref".to_string());
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();

    let report = execute_suite(&registry, SuiteKind::Smoke, &config);
    assert_eq!(report.cases.len(), 1);
    let case = &report.cases[0];

    assert_ne!(
        case.status, "unstable",
        "Elementwise add should not be unstable"
    );
    let stats = case.metrics.get("wall_ns").expect("Missing wall_ns");
    assert!(
        stats.determinism_cv.is_some(),
        "determinism_cv should be populated"
    );
    assert!(
        stats.determinism_cv.unwrap() < 0.05,
        "CV should be low for stable case"
    );
}

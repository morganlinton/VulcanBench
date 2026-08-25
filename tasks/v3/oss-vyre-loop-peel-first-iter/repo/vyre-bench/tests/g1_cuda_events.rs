//! CUDA events test.
#![allow(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_cuda_events_populated() {
    let mut config = RunConfig::default();
    config.measured_samples = Some(30);
    config.backend_id = Some("cuda".to_string());
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];
    let registry = vyre_bench::registry::collect_all();

    let report = execute_suite(&registry, SuiteKind::Smoke, &config);
    assert_eq!(
        report.cases.len(),
        1,
        "CUDA event test must execute exactly one case; empty reports indicate broken case selection or backend acquisition"
    );

    let case = &report.cases[0];
    assert_eq!(
        case.status, "pass",
        "CUDA event benchmark failed before timing assertions: {:?}",
        case.correctness
    );
    let metrics = &case.metrics;

    assert!(
        metrics.contains_key("kernel_queue_submit_ns"),
        "Queue submit NS missing"
    );
    assert!(
        metrics.contains_key("kernel_execute_ns"),
        "Kernel execute NS missing"
    );
    assert!(
        metrics.contains_key("device_sync_ns"),
        "Device sync NS missing"
    );

    let submit = metrics.get("kernel_queue_submit_ns").unwrap();
    assert!(submit.p50 > 0, "Queue submit time should be > 0");

    let exec = metrics.get("kernel_execute_ns").unwrap();
    assert!(exec.p50 > 0, "Kernel execute time should be > 0");
}

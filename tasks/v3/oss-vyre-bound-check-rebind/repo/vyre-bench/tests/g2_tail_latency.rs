//! Tail latency test.
#![allow(missing_docs, clippy::field_reassign_with_default)]

use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_tail_latency_monotonicity() {
    let mut config = RunConfig::default();
    config.measured_samples = Some(100);
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];
    let registry = vyre_bench::registry::collect_all();

    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    assert_eq!(report.cases.len(), 1);
    let case = &report.cases[0];

    let stats = case.metrics.get("wall_ns").expect("Missing wall_ns");
    assert!(
        stats.p999 >= stats.p99,
        "p999 ({}) < p99 ({})",
        stats.p999,
        stats.p99
    );
    assert!(
        stats.p99 >= stats.p95,
        "p99 ({}) < p95 ({})",
        stats.p99,
        stats.p95
    );
    assert!(
        stats.p95 >= stats.p90,
        "p95 ({}) < p90 ({})",
        stats.p95,
        stats.p90
    );
    assert!(
        stats.p90 >= stats.p50,
        "p90 ({}) < p50 ({})",
        stats.p90,
        stats.p50
    );
    assert!(
        stats.p9999 >= stats.p999,
        "p9999 ({}) < p999 ({})",
        stats.p9999,
        stats.p999
    );
}

//! Throughput consistency test.
#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_throughput_consistency() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    let mut executed_cases = 0;
    for case in report.cases {
        if case.status == "failed" {
            continue;
        }

        let device_gb_s = case
            .metrics
            .get("device_gb_s_x1000")
            .map(|m| m.p50 as f64 / 1000.0);

        if let Some(device_gb_s) = device_gb_s {
            assert!(
                device_gb_s > 0.0,
                "Case {} must report positive device throughput",
                case.id
            );
            let wall_ns = case
                .metrics
                .get("wall_ns")
                .expect("throughput case must report wall_ns")
                .p50;
            let dispatch_ns = case
                .metrics
                .get("dispatch_ns")
                .expect("throughput case must report dispatch_ns")
                .p50;
            assert!(
                dispatch_ns <= wall_ns,
                "Case {} reported dispatch_ns above wall_ns: dispatch_ns={}, wall_ns={}",
                case.id,
                dispatch_ns,
                wall_ns
            );

            if let Some(wall_gb_s) = case
                .metrics
                .get("wall_gb_s_x1000")
                .map(|m| m.p50 as f64 / 1000.0)
            {
                let limit = device_gb_s * 1.05;
                assert!(
                wall_gb_s <= limit,
                "Case {} reported wall-clock throughput above device-time throughput: wall_gb_s={}, device_gb_s={}",
                case.id,
                wall_gb_s,
                device_gb_s
            );
            }
            executed_cases += 1;
        }
    }

    assert!(
        executed_cases > 0,
        "throughput consistency must execute at least one case with throughput metrics"
    );
}

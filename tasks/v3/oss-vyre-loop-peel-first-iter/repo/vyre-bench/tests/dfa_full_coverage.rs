//! B-6 regression test  -  DFA full-coverage validation.
//!
//! Asserts that the GPU DFA match count equals the CPU baseline match count
//! for a synthetic input with a known number of planted needles.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::case::Correctness;
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[test]
fn test_dfa_gpu_matches_cpu_baseline() {
    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(30);
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.dfa_match.256k".to_string()];

    let registry = vyre_bench::registry::collect_all();
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);

    assert!(!report.cases.is_empty(), "DFA case must produce a result");
    let case = &report.cases[0];
    assert_eq!(
        case.status, "pass",
        "DFA case must pass with exact parity between GPU and CPU: status={}",
        case.status
    );

    // Verify exact correctness (GPU output == CPU baseline output)
    assert!(
        matches!(case.correctness, Correctness::Exact),
        "DFA case must report Correctness::Exact, got {:?}",
        case.correctness
    );
}

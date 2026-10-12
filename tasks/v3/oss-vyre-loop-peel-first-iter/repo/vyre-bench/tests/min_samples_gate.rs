//! B-4  -  Min samples gate.
//!
//! Verifies that the bench harness rejects runs with fewer than 30
//! measured samples for deterministic cases unless VYRE_ALLOW_FEW_SAMPLES
//! is set.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

#[allow(unsafe_code)]
#[test]
fn test_few_samples_rejected() {
    // Ensure the env var is NOT set so the rejection actually fires.
    // SAFETY: bench tests own this env var entirely. Single-threaded
    // mutation inside #[test] is the supported contract.
    unsafe {
        std::env::remove_var("VYRE_ALLOW_FEW_SAMPLES");
    }

    let mut config = RunConfig::default();
    config.warmup_samples = 1;
    config.measured_samples = Some(10); // Below 30 minimum
    config.determinism_runs = 1;
    config.case_ids = vec!["foundation.elementwise.add.1m".to_string()];

    let registry = vyre_bench::registry::collect_all();
    // The harness used to panic on the gate; it now records the
    // rejection in `report.cases[*].status` so the dispatch can keep
    // running other cases. Either contract is acceptable  -  what the
    // gate guarantees is that few-sample runs do NOT silently pass.
    let report = execute_suite(&registry, SuiteKind::Smoke, &config);
    assert!(
        !report.cases.is_empty(),
        "expected at least one rejected case in the report"
    );
    let case = &report.cases[0];
    assert_ne!(
        case.status, "pass",
        "few-sample run must not silently pass; got status={}",
        case.status
    );
    // The reason string must mention the gate so operators can act.
    let detail = format!("{:?}", case);
    assert!(
        detail.contains("measured_samples must be >= 30"),
        "rejection reason must reference the >=30 sample gate; got: {}",
        detail
    );
}

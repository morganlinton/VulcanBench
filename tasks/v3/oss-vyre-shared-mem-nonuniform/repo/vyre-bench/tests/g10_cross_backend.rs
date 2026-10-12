//! G10  -  Cross-backend matrix.
//!
//! Verifies that registered dispatch-capable backends can each run
//! the elementwise case independently, producing consistent results.

#![allow(clippy::field_reassign_with_default)]
use vyre_bench::api::suite::SuiteKind;
use vyre_bench::runner::{execute_suite, RunConfig};

const ELEMENTWISE_CASE_ID: &str = "foundation.elementwise.add.1m";

#[test]
fn test_cross_backend_elementwise() {
    let registry = vyre_bench::registry::collect_all();

    // Get the list of dispatch-capable backends
    let backends: Vec<&str> = vyre_driver::backend::registered_backends_by_precedence_slice()
        .iter()
        .filter(|reg| vyre_driver::backend::backend_dispatches(reg.id))
        .map(|reg| reg.id)
        .collect();

    if backends.is_empty() {
        panic!(
            "g10_cross_backend: no dispatch-capable backend registered. Fix: enable and register a real GPU backend for this test lane; do not treat missing GPU features as a skip."
        );
    }

    let mut results = Vec::new();
    for backend_id in &backends {
        let mut config = RunConfig::default();
        config.warmup_samples = 1;
        config.measured_samples = Some(30);
        config.determinism_runs = 1;
        config.backend_id = Some(backend_id.to_string());
        config.case_ids = vec![ELEMENTWISE_CASE_ID.to_string()];

        let report = execute_suite(&registry, SuiteKind::Smoke, &config);

        assert_eq!(
            report.cases.len(),
            1,
            "backend {backend_id} must produce exactly one benchmark case for {ELEMENTWISE_CASE_ID}. Fix: keep the smoke suite registry wired for every dispatch-capable backend instead of silently dropping or duplicating the case."
        );

        let case = &report.cases[0];
        assert_eq!(
            case.id, ELEMENTWISE_CASE_ID,
            "backend {backend_id} must preserve the requested benchmark case id"
        );
        assert_eq!(
            case.backend_id.as_deref(),
            Some(*backend_id),
            "benchmark report must retain the selected backend id for {ELEMENTWISE_CASE_ID}"
        );
        results.push((backend_id.to_string(), case.status.clone()));
    }

    // At least one backend should produce a result
    assert_eq!(
        results.len(),
        backends.len(),
        "Every dispatch-capable backend must produce results for elementwise.add"
    );

    // All backends that produced a result should pass
    for (backend, status) in &results {
        assert_ne!(
            status, "failed",
            "Backend {} should not fail elementwise.add",
            backend
        );
    }
}

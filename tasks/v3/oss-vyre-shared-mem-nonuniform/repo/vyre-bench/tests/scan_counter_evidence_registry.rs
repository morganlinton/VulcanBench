//! Scan counter evidence registry test suite.

const COUNTERS: &str = include_str!("../../docs/optimization/SCAN_COUNTER_EVIDENCE.toml");

#[test]
fn scan_counter_evidence_registry_records_required_counters() {
    for required in [
        "memory_bytes",
        "branch_divergence_proxy",
        "occupancy_proxy",
        "candidate_count",
        "counter_source",
        "unavailable_reason",
        "backend_id = \"cuda\"",
        "backend_id = \"metal\"",
        "backend_id = \"wgpu\"",
    ] {
        assert!(
            COUNTERS.contains(required),
            "Fix: scan counter evidence registry must include `{required}`"
        );
    }
}

#[test]
fn scan_counter_evidence_registry_requires_sources_or_unavailable_reasons() {
    for required in [
        "nsight-compute-or-unavailable-reason",
        "metal-counters-or-unavailable-reason",
        "timestamp-query-or-unavailable-reason",
        "unavailable_reason_required = true",
    ] {
        assert!(
            COUNTERS.contains(required),
            "Fix: scan counter evidence registry must include `{required}`"
        );
    }
    assert_eq!(
        COUNTERS
            .matches("evidence_path = \"vyre-bench/tests/scan_counter_evidence_registry.rs\"")
            .count(),
        COUNTERS.matches("[[backend]]").count(),
        "Fix: every scan counter backend row must point at this proof gate"
    );
}

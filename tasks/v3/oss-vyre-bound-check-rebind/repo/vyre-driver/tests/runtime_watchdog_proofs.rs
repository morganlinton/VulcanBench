//! Runtime watchdog proofs test suite.

const PROOFS: &str = include_str!("../../docs/optimization/RUNTIME_WATCHDOG_PROOFS.toml");

#[test]
fn runtime_watchdog_proofs_gate_resident_launches() {
    for required in [
        "queue_class",
        "bounded_drain_steps",
        "resident_time_limit_us",
        "preemption_blocker",
        "remediation_text",
        "launch_allowed",
    ] {
        assert!(
            PROOFS.contains(required),
            "runtime watchdog proof must include {required}"
        );
    }

    assert!(PROOFS.contains("frontier-expansion-unbounded"));
    assert!(PROOFS.contains("launch_allowed = false"));
}

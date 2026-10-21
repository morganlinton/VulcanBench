//! Metal hazard certificates test suite.

const CERTIFICATES: &str = include_str!("../../docs/optimization/METAL_HAZARD_CERTIFICATES.toml");

#[test]
fn metal_hazard_certificates_gate_dispatch_with_counter_waits() {
    for required in [
        "resource_id",
        "access_mode",
        "dependency_edge",
        "hazard_tracking_mode",
        "synchronization_evidence",
        "counter_wait_reason",
        "dispatch_allowed",
    ] {
        assert!(
            CERTIFICATES.contains(required),
            "Metal hazard certificate must include {required}"
        );
    }

    assert!(CERTIFICATES.contains("dispatch_allowed = false"));
    assert!(CERTIFICATES.contains("resource-hazard"));
}

//! Wgpu subgroup scan plan registry test suite.

const PLANS: &str = include_str!("../../docs/optimization/WGPU_SUBGROUP_SCAN_PLANS.toml");

#[test]
fn wgpu_subgroup_scan_plan_registry_records_features_and_routes() {
    for required in [
        "SUBGROUP",
        "SUBGROUP_BARRIER",
        "subgroup_ballot",
        "subgroup_shuffle",
        "subgroup_reduce",
        "route_id = \"native_wgpu_subgroup_scan\"",
        "route_id = \"workgroup_shared_memory_scan\"",
        "fallback_route = \"workgroup_shared_memory_scan\"",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: WGPU subgroup scan plan registry must include `{required}`"
        );
    }
}

#[test]
fn wgpu_subgroup_scan_plan_registry_requires_parity_and_diagnostics() {
    assert_eq!(
        PLANS.matches("parity_required = true").count(),
        PLANS.matches("[[route]]").count(),
        "Fix: every WGPU subgroup route must require CPU parity"
    );
    for required in [
        "VYRE_WGPU_SUBGROUP_UNSUPPORTED",
        "subgroup_min",
        "subgroup_max",
        "feature_flag",
        "Use the portable workgroup-memory route",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: WGPU subgroup scan diagnostics must include `{required}`"
        );
    }
    assert_eq!(
        PLANS
            .matches("evidence_path = \"vyre-driver-wgpu/tests/wgpu_subgroup_scan_plan_registry.rs\"")
            .count(),
        PLANS.matches("[[route]]").count() + PLANS.matches("[[diagnostic]]").count(),
        "Fix: every WGPU subgroup registry row must point at this proof gate"
    );
}

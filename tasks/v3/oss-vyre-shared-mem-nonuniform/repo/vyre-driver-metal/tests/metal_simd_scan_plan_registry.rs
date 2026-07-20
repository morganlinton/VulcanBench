//! Metal simd scan plan registry test suite.

const PLANS: &str = include_str!("../../docs/optimization/METAL_SIMD_SCAN_PLANS.toml");

#[test]
fn metal_simd_scan_plan_registry_records_metrics_and_routes() {
    for required in [
        "simdgroup_width",
        "divergence_class",
        "threadgroup_layout",
        "gpu_ns",
        "counter_source",
        "cpu_parity",
        "route_id = \"metal_simdgroup_scan\"",
        "route_id = \"metal_threadgroup_scan\"",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: Metal SIMD scan plan registry must include `{required}`"
        );
    }
}

#[test]
fn metal_simd_scan_plan_registry_requires_parity_counters_and_fallbacks() {
    assert_eq!(
        PLANS.matches("parity_required = true").count(),
        PLANS.matches("[[route]]").count(),
        "Fix: every Metal scan route must require CPU parity"
    );
    for required in [
        "metal-counters-or-unavailable-reason",
        "fallback_route = \"metal_threadgroup_scan\"",
        "fallback_route = \"cpu_ref\"",
        "VYRE_METAL_SIMDGROUP_SCAN_UNSUPPORTED",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: Metal SIMD scan plan registry must include `{required}`"
        );
    }
    assert_eq!(
        PLANS
            .matches("evidence_path = \"vyre-driver-metal/tests/metal_simd_scan_plan_registry.rs\"")
            .count(),
        PLANS.matches("[[route]]").count() + PLANS.matches("[[diagnostic]]").count(),
        "Fix: every Metal SIMD scan row must point at this proof gate"
    );
}

//! Roofline counter evidence test suite.

const ROOFLINE: &str =
    include_str!("../../docs/optimization/ROOFLINE_COUNTER_EVIDENCE.toml");

#[test]
fn roofline_counter_evidence_records_intensity_counters_bounds_and_route_explanations() {
    for required in [
        "kernel_id",
        "backend",
        "arithmetic_intensity",
        "memory_bytes",
        "operation_count",
        "achieved_throughput",
        "roofline_bound",
        "limiting_resource",
        "counter_sources",
        "route_explanation",
    ] {
        assert!(
            ROOFLINE.contains(required),
            "roofline counter evidence must include {required}"
        );
    }
}

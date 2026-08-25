//! Apple math comparators test suite.

const COMPARATORS: &str = include_str!("../../docs/optimization/APPLE_MATH_COMPARATORS.toml");

#[test]
fn apple_math_comparators_record_backend_reasons_and_counters() {
    for required in [
        "mpsgraph",
        "vyre_msl",
        "cpu_reference",
        "kernel_family",
        "output_digest",
        "compile_ns",
        "gpu_ns",
        "counter_evidence",
        "selected_backend_reason",
    ] {
        assert!(
            COMPARATORS.contains(required),
            "Apple math comparator must include {required}"
        );
    }
}

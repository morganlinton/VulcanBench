//! Coverage sanitizer matrix test suite.

const MATRIX: &str = include_str!("../../docs/optimization/COVERAGE_SANITIZER_MATRIX.toml");

#[test]
fn coverage_sanitizer_matrix_records_modes_sanitizers_and_failure_policies() {
    for required in [
        "function",
        "basic_block",
        "edge",
        "source_region",
        "address",
        "undefined_behavior",
        "leak",
        "thread",
        "memory",
        "oom_policy",
        "timeout_policy",
        "crash_reproducer",
        "report_link",
    ] {
        assert!(
            MATRIX.contains(required),
            "coverage sanitizer matrix must include {required}"
        );
    }
}

//! Regex cpu gpu partition registry test suite.

const REGISTRY: &str = include_str!("../../docs/optimization/REGEX_CPU_GPU_PARTITION.toml");

const REQUIRED_ROUTES: &[&str] = &[
    "all_cpu",
    "all_gpu_eligible",
    "mixed_prefilter_verify",
    "verifier_only",
    "rejected",
];
const REQUIRED_METRICS: &[&str] = &[
    "transfer_bytes",
    "active_ns",
    "match_parity",
    "unsupported_count",
    "verifier_load",
];

#[test]
fn regex_cpu_gpu_partition_registry_covers_routes_and_metrics() {
    for route in REQUIRED_ROUTES {
        assert!(
            REGISTRY.contains(&format!("route_id = \"{route}\"")),
            "Fix: CPU/GPU regex partition registry must include route `{route}`"
        );
    }
    for metric in REQUIRED_METRICS {
        assert!(
            REGISTRY.contains(&format!("\"{metric}\"")),
            "Fix: CPU/GPU regex partition registry must require metric `{metric}`"
        );
    }
}

#[test]
fn regex_cpu_gpu_partition_registry_records_transfer_verifier_and_failure_modes() {
    for required in [
        "transfer_policy = \"no_gpu_transfer\"",
        "transfer_policy = \"upload_pattern_database_and_haystack\"",
        "transfer_policy = \"candidate_ranges_only\"",
        "verifier_policy = \"required\"",
        "failure_mode = \"transfer_dominated\"",
        "failure_mode = \"verifier_saturation\"",
        "failure_mode = \"nonpartitionable_pattern\"",
    ] {
        assert!(
            REGISTRY.contains(required),
            "Fix: CPU/GPU regex partition registry must include `{required}`"
        );
    }
}

#[test]
fn regex_cpu_gpu_partition_registry_requires_parity_for_executable_routes() {
    assert_eq!(
        REGISTRY.matches("parity_required = true").count(),
        4,
        "Fix: all executable CPU/GPU routes must require output parity"
    );
    assert_eq!(
        REGISTRY
            .matches("evidence_path = \"vyre-bench/tests/regex_cpu_gpu_partition_registry.rs\"")
            .count(),
        REGISTRY.matches("[[route]]").count(),
        "Fix: every CPU/GPU partition row must point at this proof gate"
    );
}

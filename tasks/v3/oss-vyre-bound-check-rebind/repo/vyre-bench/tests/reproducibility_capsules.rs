//! Reproducibility capsules test suite.

const CAPSULES: &str = include_str!("../../docs/optimization/REPRODUCIBILITY_CAPSULES.toml");

#[test]
fn reproducibility_capsules_bind_inputs_environment_hardware_counters_and_outputs() {
    for required in [
        "capsule_id",
        "input_digest",
        "environment_digest",
        "hardware_fingerprint",
        "counter_schema",
        "output_digest",
        "capability_digest",
        "private_path_redaction",
        "cuda-scan-bench-capsule",
        "metal-ranking-bench-capsule",
    ] {
        assert!(
            CAPSULES.contains(required),
            "reproducibility capsule must include {required}"
        );
    }
}

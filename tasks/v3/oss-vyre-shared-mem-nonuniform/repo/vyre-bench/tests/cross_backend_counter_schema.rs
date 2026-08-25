//! Cross backend counter schema test suite.

const SCHEMA: &str = include_str!("../../docs/optimization/CROSS_BACKEND_COUNTER_SCHEMA.toml");

#[test]
fn cross_backend_counter_schema_requires_source_units_and_availability() {
    for required in [
        "source",
        "unit",
        "kernel_id",
        "command_id",
        "timestamp_availability",
        "memory_bytes",
        "occupancy_proxy",
        "unavailable_reason",
        "cuda",
        "wgpu",
        "metal",
        "cpu",
        "dpu",
        "fpga",
    ] {
        assert!(
            SCHEMA.contains(required),
            "cross-backend counter schema must include {required}"
        );
    }
}

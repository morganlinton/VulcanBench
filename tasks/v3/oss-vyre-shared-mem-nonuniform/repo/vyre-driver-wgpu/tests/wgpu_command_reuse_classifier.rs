//! Wgpu command reuse classifier test suite.

const CLASSIFIER: &str = include_str!("../../docs/optimization/WGPU_COMMAND_REUSE_CLASSIFIER.toml");

#[test]
fn wgpu_command_reuse_requires_stable_topology_and_resources() {
    for required in [
        "bind_group_digest",
        "pipeline_id",
        "buffer_shape",
        "timestamp_capability",
        "topology_digest",
        "invalidation_reason",
        "reuse_allowed",
    ] {
        assert!(
            CLASSIFIER.contains(required),
            "WGPU command reuse classifier must include {required}"
        );
    }

    assert!(CLASSIFIER.contains("reuse_allowed = false"));
}

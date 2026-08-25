//! Backend capability digests test suite.

const DIGESTS: &str = include_str!("../../docs/optimization/BACKEND_CAPABILITY_DIGESTS.toml");

#[test]
fn backend_capability_digests_unify_docs_dispatch_and_benchmarks() {
    for required in [
        "device_id",
        "feature_flags",
        "timing_capability",
        "graph_capability",
        "resident_capability",
        "unsupported_reasons",
        "capability_digest",
    ] {
        assert!(
            DIGESTS.contains(required),
            "backend capability digest must include {required}"
        );
    }

    assert!(DIGESTS.contains("cuda-rtx5090-capabilities"));
    assert!(DIGESTS.contains("wgpu-vulkan-generic-capabilities"));
}

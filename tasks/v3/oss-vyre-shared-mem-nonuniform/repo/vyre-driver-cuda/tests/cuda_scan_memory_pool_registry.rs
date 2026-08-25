//! Cuda scan memory pool registry test suite.

const POOLS: &str = include_str!("../../docs/optimization/CUDA_SCAN_MEMORY_POOLS.toml");

const REQUIRED_POOLS: &[&str] = [
    "compile_scratch",
    "scan_scratch",
    "pattern_database",
    "graph_capture_staging",
]
.as_slice();

#[test]
fn cuda_scan_memory_pool_registry_covers_required_pools() {
    for pool in REQUIRED_POOLS {
        assert!(
            POOLS.contains(&format!("pool_id = \"{pool}\"")),
            "Fix: CUDA scan memory pool registry must include pool `{pool}`"
        );
    }
}

#[test]
fn cuda_scan_memory_pool_registry_requires_stream_order_and_capture_evidence() {
    assert_eq!(
        POOLS.matches("stream_ordered = true").count(),
        POOLS.matches("[[pool]]").count(),
        "Fix: every CUDA scan memory pool must be stream ordered"
    );
    assert!(
        POOLS.matches("capture_eligible = true").count() >= 3,
        "Fix: scan scratch, pattern database, and graph capture staging pools must be capture eligible"
    );
    for required in [
        "max_compile_scratch_bytes",
        "max_scan_scratch_bytes",
        "max_pattern_database_bytes",
        "max_graph_capture_staging_bytes",
        "VYRE_CUDA_SCAN_GRAPH_CAPTURE_POOL_MISS",
    ] {
        assert!(
            POOLS.contains(required),
            "Fix: CUDA scan memory pool registry must include `{required}`"
        );
    }
}

#[test]
fn cuda_scan_memory_pool_registry_rows_point_to_proof_gate() {
    assert_eq!(
        POOLS
            .matches("evidence_path = \"vyre-driver-cuda/tests/cuda_scan_memory_pool_registry.rs\"")
            .count(),
        POOLS.matches("[[pool]]").count(),
        "Fix: every CUDA scan memory pool row must point at this proof gate"
    );
}

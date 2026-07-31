//! Ann research planners test suite.

const ANN_ADAPTERS: &str = include_str!("../../docs/optimization/ANN_COMPARATOR_ADAPTERS.toml");
const SSD_PLANNER: &str = include_str!("../../docs/optimization/SSD_AWARE_ANN_PLANNER.toml");
const GPU_DIRECT: &str = include_str!("../../docs/optimization/GPU_DIRECT_CORPUS_PAGING_CAPABILITIES.toml");
const QUANTIZATION: &str = include_str!("../../docs/optimization/QUANTIZATION_CONTRACTS.toml");
const CONSTRAINED: &str = include_str!("../../docs/optimization/CONSTRAINED_ANN_PLANNER.toml");

#[test]
fn ann_comparator_adapters_record_recall_cost_and_route_reason() {
    for required in [
        "cagra",
        "vamana",
        "ivf",
        "pq",
        "exact_small_fixture_parity",
        "recall_at_k",
        "build_ns",
        "query_ns",
        "memory_bytes",
        "selected_route_reason",
    ] {
        assert!(ANN_ADAPTERS.contains(required), "ANN adapters must include {required}");
    }
}

#[test]
fn storage_aware_ann_and_gpu_direct_records_blockers_and_parity() {
    for required in [
        "candidate_prefetch",
        "queue_depth",
        "graph_cache_hit",
        "read_bytes",
        "recall_floor",
        "p99_latency_ns",
        "blocker_reason",
    ] {
        assert!(SSD_PLANNER.contains(required), "SSD planner must include {required}");
    }

    for required in [
        "filesystem",
        "alignment",
        "device",
        "driver",
        "fallback_diagnostic",
        "cpu_bounce_bytes_avoided",
        "output_or_recall_parity",
        "VYRE_GDS_ALIGNMENT_UNSUPPORTED",
    ] {
        assert!(GPU_DIRECT.contains(required), "GPU-direct planner must include {required}");
    }
}

#[test]
fn quantization_and_constrained_ann_publish_numeric_and_predicate_bounds() {
    for required in [
        "source_type",
        "quantizer_id",
        "accumulator_type",
        "error_bound",
        "recall_floor",
        "storage_bytes",
        "exact_small_fixture_parity",
    ] {
        assert!(QUANTIZATION.contains(required), "quantization contract must include {required}");
    }

    for required in [
        "predicate_automata",
        "vector_route",
        "candidate_count",
        "precision",
        "recall",
        "verifier_proof",
        "blocker_reason",
    ] {
        assert!(CONSTRAINED.contains(required), "constrained ANN planner must include {required}");
    }
}

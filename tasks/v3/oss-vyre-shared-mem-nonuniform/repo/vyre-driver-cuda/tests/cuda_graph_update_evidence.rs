//! Cuda graph update evidence test suite.

const EVIDENCE: &str = include_str!("../../docs/optimization/CUDA_GRAPH_UPDATE_EVIDENCE.toml");

#[test]
fn cuda_graph_update_evidence_distinguishes_update_from_recapture() {
    for required in [
        "topology_id",
        "node_parameter_diff",
        "pointer_identity",
        "shape_digest",
        "replay_count",
        "update-node-params",
        "recapture",
        "output_digest",
    ] {
        assert!(
            EVIDENCE.contains(required),
            "CUDA graph update evidence must include {required}"
        );
    }
}

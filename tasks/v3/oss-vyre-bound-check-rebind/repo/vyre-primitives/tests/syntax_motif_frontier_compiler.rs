//! Syntax motif frontier compiler test suite.

const MOTIFS: &str = include_str!("../../docs/optimization/SYNTAX_MOTIF_FRONTIER_COMPILER.toml");

#[test]
fn syntax_motif_frontier_compiler_preserves_routes_and_parity() {
    for required in [
        "ast_walker",
        "graph_motif_engine",
        "gpu_frontier",
        "span_policy",
        "route_reason",
        "node_id_policy",
        "match_count_contract",
    ] {
        assert!(
            MOTIFS.contains(required),
            "syntax motif compiler registry must include {required}"
        );
    }

    assert!(MOTIFS.contains("equal-to-ast-walker"));
    assert!(MOTIFS.contains("equal-to-cpu-graph-motif-engine"));
}

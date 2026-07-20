use super::*;

#[test]
fn gpu_semantic_lowering_completes_for_scope_nesting() {
    let fix = fixture_scope_nesting();
    let typed = classify(&fix);
    let (gpu_nodes, gpu_edges) = run_gpu_semantic_lower(&typed);

    let node_count = node_count_from_vast(&typed) as usize;
    assert_eq!(
        gpu_nodes.len(),
        node_count * C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize * 4,
        "semantic node buffer size must match node count"
    );
    assert_eq!(
        gpu_edges.len(),
        node_count * C_AST_PG_EDGE_ROWS_PER_NODE as usize * C_AST_PG_EDGE_STRIDE_U32 as usize * 4,
        "semantic edge buffer size must match node count"
    );
}

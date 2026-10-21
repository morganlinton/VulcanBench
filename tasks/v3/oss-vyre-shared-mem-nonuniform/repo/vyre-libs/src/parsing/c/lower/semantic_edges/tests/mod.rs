use super::*;

#[test]
fn resolved_semantic_edges_reads_goto_edge_fields() {
    let mut vast = vec![0u32; VAST_NODE_STRIDE_U32 as usize * 3];
    vast[IDX_KIND] = C_AST_KIND_GOTO_STMT;
    vast[IDX_PARENT] = u32::MAX;
    vast[IDX_NEXT_SIBLING] = 1;
    vast[VAST_NODE_STRIDE_U32 as usize + IDX_PARENT] = 0;
    vast[VAST_NODE_STRIDE_U32 as usize + IDX_SYMBOL_HASH] = 0xA11CE;
    let label_base = VAST_NODE_STRIDE_U32 as usize * 2;
    vast[label_base + IDX_KIND] = C_AST_KIND_LABEL_STMT;
    vast[label_base + IDX_PARENT] = 0;
    vast[label_base + IDX_SYMBOL_HASH] = 0xA11CE;

    let (edge, extra) = resolved_semantic_edges(&vast, 0, 3, C_AST_KIND_GOTO_STMT);

    assert_eq!(edge.kind, C_AST_PG_EDGE_GOTO_TARGET);
    assert_eq!(edge.src, 0);
    assert_eq!(edge.dst, 2);
    assert_eq!(extra.kind, C_AST_PG_EDGE_NONE);
}

#[test]
#[should_panic(expected = "truncated VAST")]
fn resolved_semantic_edges_rejects_truncated_vast_rows() {
    let _ = resolved_semantic_edges(&[], 0, 1, C_AST_KIND_GOTO_STMT);
}

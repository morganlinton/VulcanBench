use super::semantic_features::SemanticFeatureInputs;
use super::semantic_haystack::SemanticHaystack;
use super::*;

pub(in crate::pipeline) struct C11SemanticGraphs {
    pub(in crate::pipeline) vast_blob: Vec<u8>,
    pub(in crate::pipeline) expr_shape_blob: Vec<u8>,
    pub(in crate::pipeline) pg_blob: Vec<u8>,
    pub(in crate::pipeline) sema_blob: Vec<u8>,
    pub(in crate::pipeline) semantic_pg_nodes: Vec<u8>,
    pub(in crate::pipeline) semantic_pg_edges: Vec<u8>,
    pub(in crate::pipeline) vast_bytes: u64,
    pub(in crate::pipeline) expression_shape_bytes: u64,
    pub(in crate::pipeline) program_graph_bytes: u64,
    pub(in crate::pipeline) semantic_node_bytes: u64,
    pub(in crate::pipeline) semantic_edge_bytes: u64,
    pub(in crate::pipeline) sema_scope_bytes: u64,
}

pub(in crate::pipeline) fn build_c11_semantic_graphs(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &[u8],
    tok_types: &[u32],
    start_words: &[u32],
    len_words: &[u32],
    types_logical: &[u8],
    starts_logical: &[u8],
    lens_logical: &[u8],
    semantic_haystack: &SemanticHaystack<'_>,
    nt: u32,
    readback_terminal_outputs: bool,
    semantic_features: &SemanticFeatureInputs,
    mut log: impl FnMut(&str),
) -> Result<C11SemanticGraphs, String> {
    let vast_pg = build_vast_and_pg(
        backend,
        path,
        types_logical,
        starts_logical,
        lens_logical,
        source,
        semantic_haystack.bytes,
        semantic_haystack.len,
        nt,
        semantic_haystack.packed,
        readback_terminal_outputs,
        semantic_features.resolve_control_edges,
        semantic_features.resolve_conditional_shapes,
        semantic_features.global_typedef_hash_bytes.as_deref(),
    )?;
    log("build_vast_and_pg");
    let sema_scope = build_sema_scope(
        backend,
        path,
        tok_types,
        start_words,
        len_words,
        source,
        types_logical,
        starts_logical,
        lens_logical,
        semantic_haystack.bytes,
        semantic_haystack.len,
        nt,
        semantic_haystack.packed,
        readback_terminal_outputs,
    )?;
    log("build_sema_scope");
    Ok(C11SemanticGraphs {
        vast_bytes: vast_pg.typed_vast_bytes,
        expression_shape_bytes: vast_pg.expression_shape_bytes,
        program_graph_bytes: vast_pg.program_graph_bytes,
        semantic_node_bytes: vast_pg.semantic_node_bytes,
        semantic_edge_bytes: vast_pg.semantic_edge_bytes,
        sema_scope_bytes: sema_scope.byte_len,
        vast_blob: vast_pg.typed_vast_blob,
        expr_shape_blob: vast_pg.expression_shape_blob,
        pg_blob: vast_pg.program_graph_blob,
        semantic_pg_nodes: vast_pg.semantic_pg_nodes,
        semantic_pg_edges: vast_pg.semantic_pg_edges,
        sema_blob: sema_scope.blob,
    })
}

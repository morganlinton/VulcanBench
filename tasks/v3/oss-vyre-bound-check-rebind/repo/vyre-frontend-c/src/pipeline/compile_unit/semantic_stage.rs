use super::*;

pub(super) struct ObjectSemantics {
    pub(super) vast_blob: Vec<u8>,
    pub(super) expr_shape_blob: Vec<u8>,
    pub(super) pg_blob: Vec<u8>,
    pub(super) sema_blob: Vec<u8>,
    pub(super) semantic_pg_nodes: Vec<u8>,
    pub(super) semantic_pg_edges: Vec<u8>,
}

pub(super) fn build_object_semantics(
    backend: &dyn VyreBackend,
    path: &Path,
    source: &str,
    lexed: &lex_stage::ObjectLexTokens,
    decoded: &token_decode::DecodedObjectTokens,
    expanded_haystack_cache: &mut Option<(Vec<u8>, u32)>,
    trace: &mut trace::CompileTrace,
) -> Result<ObjectSemantics, String> {
    let semantic_haystack = select_semantic_haystack(
        source,
        expanded_haystack_cache,
        lexed
            .cuda_keyword_haystack
            .as_ref()
            .map(|(bytes, len)| (bytes.as_slice(), *len)),
        |stage| trace.log(stage),
    )?;
    let semantic_features = build_semantic_feature_inputs(
        source.as_bytes(),
        &decoded.tok_types,
        &decoded.start_words,
        &decoded.len_words,
    );
    let semantic_graphs = build_c11_semantic_graphs(
        backend,
        path,
        source.as_bytes(),
        &decoded.tok_types,
        &decoded.start_words,
        &decoded.len_words,
        &decoded.types_logical,
        &decoded.starts_logical,
        &decoded.lens_logical,
        &semantic_haystack,
        decoded.nt,
        true,
        &semantic_features,
        |stage| trace.log(stage),
    )?;
    Ok(ObjectSemantics {
        vast_blob: semantic_graphs.vast_blob,
        expr_shape_blob: semantic_graphs.expr_shape_blob,
        pg_blob: semantic_graphs.pg_blob,
        sema_blob: semantic_graphs.sema_blob,
        semantic_pg_nodes: semantic_graphs.semantic_pg_nodes,
        semantic_pg_edges: semantic_graphs.semantic_pg_edges,
    })
}

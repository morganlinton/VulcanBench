use super::*;

pub(in crate::pipeline) struct SemanticFeatureInputs {
    pub(in crate::pipeline) global_typedef_hash_bytes: Option<Vec<u8>>,
    pub(in crate::pipeline) resolve_control_edges: bool,
    pub(in crate::pipeline) resolve_conditional_shapes: bool,
}

pub(in crate::pipeline) fn build_semantic_feature_inputs(
    source: &[u8],
    tok_types: &[u32],
    start_words: &[u32],
    len_words: &[u32],
) -> SemanticFeatureInputs {
    SemanticFeatureInputs {
        global_typedef_hash_bytes: c_global_typedef_fast_hashes(
            source,
            tok_types,
            start_words,
            len_words,
        )
        .map(|hashes| vec_u32_le_bytes(&hashes)),
        resolve_control_edges: semantic_control_edges_required(tok_types),
        resolve_conditional_shapes: conditional_expression_shapes_required(tok_types),
    }
}

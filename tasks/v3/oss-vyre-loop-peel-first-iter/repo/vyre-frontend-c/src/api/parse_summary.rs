/// Full pre-lowering C11 parser/sema evidence emitted by the GPU frontend without object/codegen
/// stages.
///
/// `parse_source`, `parse_translation_unit`, and `parse_translation_unit_bytes` require typed VAST,
/// ProgramGraph, semantic ProgramGraph, and semantic-scope evidence before returning a non-empty
/// summary. `parse_syntax_source` is the explicit syntax-only API and may leave semantic evidence
/// fields at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CParseSummary {
    /// Original source byte length.
    pub source_bytes: u64,
    /// Logical token count after keyword promotion and span repair.
    pub token_count: u32,
    /// AST evidence bytes produced by the parser stage.
    pub ast_bytes: u64,
    /// AST node count reported by parser evidence stages.
    pub ast_node_count: u32,
    /// Typed VAST evidence bytes produced by syntax lowering.
    pub vast_bytes: u64,
    /// ABI-layout evidence bytes produced by type layout stages.
    pub abi_layout_bytes: u64,
    /// Expression-shape evidence bytes produced by expression lowering.
    pub expression_shape_bytes: u64,
    /// Program-graph evidence bytes produced by syntax lowering.
    pub program_graph_bytes: u64,
    /// Semantic-node evidence bytes produced by semantic lowering.
    pub semantic_node_bytes: u64,
    /// Semantic-edge evidence bytes produced by semantic lowering.
    pub semantic_edge_bytes: u64,
    /// Scope-record evidence bytes produced by semantic scope lowering.
    pub sema_scope_bytes: u64,
    /// Function-record bytes produced by structure extraction.
    pub function_record_bytes: u64,
    /// Call-record bytes produced by structure extraction.
    pub call_record_bytes: u64,
}

impl CParseSummary {
    pub(crate) fn syntax_only(
        source_bytes: u64,
        token_count: u32,
        ast_bytes: u64,
        ast_node_count: u32,
        function_record_bytes: u64,
        call_record_bytes: u64,
    ) -> Self {
        Self {
            source_bytes,
            token_count,
            ast_bytes,
            ast_node_count,
            vast_bytes: 0,
            abi_layout_bytes: 0,
            expression_shape_bytes: 0,
            program_graph_bytes: 0,
            semantic_node_bytes: 0,
            semantic_edge_bytes: 0,
            sema_scope_bytes: 0,
            function_record_bytes,
            call_record_bytes,
        }
    }
}

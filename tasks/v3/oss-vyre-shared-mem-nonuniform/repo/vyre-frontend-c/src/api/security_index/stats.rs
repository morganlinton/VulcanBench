use super::*;

impl CObjectSecurityIndex {
    /// Build a compact count summary for release gates and corpus reports.
    #[must_use]
    pub fn stats(&self) -> CObjectSecurityStats {
        CObjectSecurityStats {
            ast_nodes: self.ast.ast_node_count,
            tokens: count_u64(self.lex.tokens.len(), "lex token count"),
            semantic_nodes: count_u64(self.semantic_graph.nodes.len(), "semantic node count"),
            semantic_edges: count_u64(self.semantic_graph.edges.len(), "semantic edge count"),
            declaration_nodes: count_u64(
                self.semantic_graph.declaration_nodes().count(),
                "semantic declaration-node count",
            ),
            control_nodes: count_u64(
                self.semantic_graph.control_nodes().count(),
                "semantic control-node count",
            ),
            gnu_nodes: count_u64(
                self.semantic_graph.gnu_nodes().count(),
                "GNU semantic-node count",
            ),
            declaration_symbols: self.sema_scope.declaration_rows,
            identifier_symbols: self.sema_scope.identifier_rows,
            abi_types: count_u64(self.abi_layout.entries.len(), "ABI type count"),
            function_records: count_u64(self.structure.functions.len(), "function record count"),
            call_records: count_u64(self.structure.calls.len(), "call record count"),
        }
    }

    /// True when the object carries enough data for non-trivial static analysis.
    #[must_use]
    pub fn has_static_analysis_surface(&self) -> bool {
        let stats = self.stats();
        stats.ast_nodes != 0
            && stats.tokens != 0
            && stats.semantic_nodes != 0
            && stats.declaration_symbols != 0
            && stats.abi_types != 0
            && stats.function_records != 0
    }

    /// True when the object carries resolved function and call-site rows.
    #[must_use]
    pub fn has_call_graph_surface(&self) -> bool {
        let stats = self.stats();
        stats.function_records != 0 && stats.call_records != 0
    }
}

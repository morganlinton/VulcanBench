use super::*;

impl CObjectSecurityIndex {
    /// Enforce the minimum object evidence needed for useful security/static analysis.
    pub fn require_static_analysis_surface(&self) -> Result<(), String> {
        let stats = self.stats();
        let mut missing = Vec::new();
        if stats.ast_nodes == 0 {
            missing.push("AST nodes");
        }
        if stats.tokens == 0 {
            missing.push("tokens");
        }
        if stats.semantic_nodes == 0 {
            missing.push("semantic graph nodes");
        }
        if stats.declaration_symbols == 0 {
            missing.push("declaration symbols");
        }
        if stats.abi_types == 0 {
            missing.push("ABI type rows");
        }
        if stats.function_records == 0 {
            missing.push("function records");
        }
        if missing.is_empty() {
            return Ok(());
        }
        Err(format!(
            "vyre-frontend-c object is missing required static-analysis evidence: {}. Fix: compile with the full GPU C frontend pipeline and preserve VYRECOB2 sections.",
            missing.join(", ")
        ))
    }

    /// Enforce call-graph evidence for workloads that should contain calls.
    pub fn require_call_graph_surface(&self) -> Result<(), String> {
        if self.has_call_graph_surface() {
            return Ok(());
        }
        let stats = self.stats();
        Err(format!(
            "vyre-frontend-c object is missing call-graph evidence: function_records={}, call_records={}. Fix: preserve Functions and Calls VYRECOB2 sections from the GPU C structure extraction pass.",
            stats.function_records, stats.call_records
        ))
    }
}

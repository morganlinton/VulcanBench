use super::*;
/// Security-analysis view decoded from a GPU-produced `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectSecurityIndex {
    /// Framed AST windows and node counts.
    pub ast: CObjectAst,
    /// Decoded token stream and source spans.
    pub lex: CObjectLexIndex,
    /// Semantic ProgramGraph nodes and edges.
    pub semantic_graph: CObjectSemanticGraph,
    /// Scope/declaration/identifier records.
    pub sema_scope: CObjectSemaScope,
    /// ABI type layout rows.
    pub abi_layout: CObjectAbiLayout,
    /// Function and call-site structure records.
    pub structure: CObjectStructureIndex,
}

/// Dense summary of static-analysis surface present in an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectSecurityStats {
    /// Total AST nodes decoded from all windows.
    pub ast_nodes: u64,
    /// Semantic graph node count.
    pub semantic_nodes: u64,
    /// Semantic graph edge count.
    pub semantic_edges: u64,
    /// Declaration-like semantic graph nodes.
    pub declaration_nodes: u64,
    /// Control-flow semantic graph nodes.
    pub control_nodes: u64,
    /// GNU-extension semantic graph nodes.
    pub gnu_nodes: u64,
    /// Scope rows carrying declarations.
    pub declaration_symbols: u64,
    /// Scope rows carrying identifiers.
    pub identifier_symbols: u64,
    /// ABI type slots.
    pub abi_types: u64,
    /// Logical token count.
    pub tokens: u64,
    /// Function records.
    pub function_records: u64,
    /// Call-site records.
    pub call_records: u64,
}

/// Resolved call edge joining a call-site row to its enclosing function record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectFunctionCall<'a> {
    /// Function-record index of the caller.
    pub caller_index: u32,
    /// Enclosing function record.
    pub caller: &'a CObjectFunctionRecord,
    /// Raw call-site record.
    pub call: &'a CObjectCallRecord,
    /// Callee source token when the call row points at a valid token.
    pub callee: Option<&'a CObjectToken>,
}

/// Symbol reference joined to its source token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectSymbolToken<'a> {
    /// Raw symbol reference decoded from semantic scope rows.
    pub symbol: CObjectSymbolRef,
    /// Matching lexical token when the symbol span resolves exactly.
    pub token: Option<&'a CObjectToken>,
}

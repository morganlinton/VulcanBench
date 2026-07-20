use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Decoded semantic ProgramGraph node row.
pub struct CAstSemanticPgNode {
    /// C AST or shared predicate node kind for this semantic node.
    pub kind: u32,
    /// Inclusive source byte offset for the node span.
    pub span_start: u32,
    /// Exclusive source byte offset for the node span.
    pub span_end: u32,
    /// Parent node index, or `u32::MAX` when the node has no parent.
    pub parent: u32,
    /// First child node index, or `u32::MAX` when absent.
    pub first_child: u32,
    /// Next sibling node index, or `u32::MAX` when absent.
    pub next_sibling: u32,
    /// Semantic category assigned by C AST to ProgramGraph lowering.
    pub category: u32,
    /// Semantic role assigned by C AST to ProgramGraph lowering.
    pub role: u32,
    /// First role-specific attribute word.
    pub attr0: u32,
    /// Second role-specific attribute word.
    pub attr1: u32,
}

impl CAstSemanticPgNode {
    /// Return true when this node belongs to the GNU-extension semantic category.
    #[must_use]
    pub fn has_builtin_role(&self) -> bool {
        self.category == C_AST_PG_CATEGORY_GNU
    }
}

/// Decoded semantic ProgramGraph edge row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CAstSemanticPgEdge {
    /// Semantic edge kind.
    pub kind: u32,
    /// Source semantic node index, or `u32::MAX` when the row is empty.
    pub source: u32,
    /// Target semantic node index, or `u32::MAX` when the row is empty.
    pub target: u32,
    /// C AST kind of the node that emitted this edge row.
    pub owner_kind: u32,
    /// Semantic role of the node that emitted this edge row.
    pub owner_role: u32,
    /// Semantic category of the node that emitted this edge row.
    pub owner_category: u32,
}

/// Decoded semantic scope row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CSemaScopeRecord {
    /// Scope id active at this token.
    pub scope_id: u32,
    /// Parent scope id, or `u32::MAX` for the root scope.
    pub parent_scope_id: u32,
    /// Declaration kind encoded by the semantic registry.
    pub decl_kind: u32,
    /// Interned identifier hash for this row, or zero when absent.
    pub identifier_id: u32,
    /// Source byte offset for the token.
    pub token_start: u32,
    /// Source byte length for the token.
    pub token_len: u32,
}

impl CSemaScopeRecord {
    /// Return true when this row carries declaration information.
    #[must_use]
    pub fn has_declaration(&self) -> bool {
        self.decl_kind != DECL_KIND_NONE
    }

    /// Return true when this row carries an interned identifier hash.
    #[must_use]
    pub fn has_identifier(&self) -> bool {
        self.identifier_id != 0
    }

    /// Return a stable human-readable declaration kind name.
    #[must_use]
    pub fn decl_kind_name(&self) -> &'static str {
        match self.decl_kind {
            DECL_KIND_FUNCTION => "function",
            DECL_KIND_FUNCTION_DECL => "function_decl",
            DECL_KIND_VARIABLE => "variable",
            DECL_KIND_LABEL => "label",
            DECL_KIND_TYPEDEF => "typedef",
            DECL_KIND_ENUM_CONSTANT => "enum_constant",
            _ => "invalid",
        }
    }
}

pub(super) fn is_known_decl_kind(kind: u32) -> bool {
    matches!(
        kind,
        DECL_KIND_NONE
            | DECL_KIND_FUNCTION
            | DECL_KIND_FUNCTION_DECL
            | DECL_KIND_VARIABLE
            | DECL_KIND_LABEL
            | DECL_KIND_TYPEDEF
            | DECL_KIND_ENUM_CONSTANT
    )
}

pub(super) fn c_ast_pg_role_name(role: u32) -> &'static str {
    match role {
        C_AST_PG_ROLE_NONE => "none",
        C_AST_PG_ROLE_LABEL => "label",
        C_AST_PG_ROLE_CASE => "case",
        C_AST_PG_ROLE_DEFAULT => "default",
        C_AST_PG_ROLE_STATEMENT_EXPR => "statement_expr",
        C_AST_PG_ROLE_INLINE_ASM => "inline_asm",
        C_AST_PG_ROLE_ASM_TEMPLATE => "asm_template",
        C_AST_PG_ROLE_ASM_OUTPUT => "asm_output",
        C_AST_PG_ROLE_ASM_INPUT => "asm_input",
        C_AST_PG_ROLE_ASM_CLOBBER => "asm_clobber",
        C_AST_PG_ROLE_ASM_GOTO_LABEL => "asm_goto_label",
        C_AST_PG_ROLE_ASM_QUALIFIER => "asm_qualifier",
        C_AST_PG_ROLE_GNU_ATTRIBUTE => "gnu_attribute",
        C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL => "gnu_attribute_detail",
        C_AST_PG_ROLE_INITIALIZER_LIST => "initializer_list",
        C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS => "field_designator_or_member_access",
        C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT => "array_designator_or_subscript",
        C_AST_PG_ROLE_RANGE_DESIGNATOR => "range_designator",
        C_AST_PG_ROLE_ASSIGNMENT => "assignment",
        C_AST_PG_ROLE_FUNCTION_DEFINITION => "function_definition",
        C_AST_PG_ROLE_FUNCTION_DECLARATOR => "function_declarator",
        C_AST_PG_ROLE_AGGREGATE_DECL => "aggregate_decl",
        C_AST_PG_ROLE_FIELD_DECL => "field_decl",
        C_AST_PG_ROLE_TYPEDEF_DECL => "typedef_decl",
        C_AST_PG_ROLE_ENUMERATOR_DECL => "enumerator_decl",
        C_AST_PG_ROLE_POINTER_DECL => "pointer_decl",
        C_AST_PG_ROLE_ARRAY_DECL => "array_decl",
        C_AST_PG_ROLE_BIT_FIELD_DECL => "bit_field_decl",
        C_AST_PG_ROLE_STATIC_ASSERT_DECL => "static_assert_decl",
        C_AST_PG_ROLE_EXPRESSION => "expression",
        C_AST_PG_ROLE_DECLARATION => "declaration",
        C_AST_PG_ROLE_GOTO => "goto",
        C_AST_PG_ROLE_SWITCH => "switch",
        C_AST_PG_ROLE_SELECTION => "selection",
        C_AST_PG_ROLE_LOOP => "loop",
        C_AST_PG_ROLE_RETURN => "return",
        C_AST_PG_ROLE_BREAK => "break",
        C_AST_PG_ROLE_CONTINUE => "continue",
        C_AST_PG_ROLE_UNREACHABLE => "unreachable",
        C_AST_PG_ROLE_ALIGNOF => "alignof",
        C_AST_PG_ROLE_FUNCTION_POINTER_DECL => "function_pointer_decl",
        _ => "invalid",
    }
}

pub(super) fn is_known_semantic_role(role: u32) -> bool {
    !matches!(c_ast_pg_role_name(role), "invalid")
}

pub(super) fn is_known_semantic_category(category: u32) -> bool {
    matches!(
        category,
        C_AST_PG_CATEGORY_NONE
            | C_AST_PG_CATEGORY_CONTROL
            | C_AST_PG_CATEGORY_EXPRESSION
            | C_AST_PG_CATEGORY_GNU
            | C_AST_PG_CATEGORY_DECLARATION
    )
}

/// Decoded framed `VYRAST1` AST payload from a `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectAst {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Per-GPU-window AST rows.
    pub windows: Vec<CObjectAstWindow>,
    /// Total AST node count across all windows.
    pub ast_node_count: u64,
}

/// One AST GPU window decoded from the framed AST section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectAstWindow {
    /// First token index covered by this window.
    pub token_start: u32,
    /// Number of tokens covered by this window.
    pub token_count: u32,
    /// Flat AST words: four `u32` words per node.
    pub ast_words: Vec<u32>,
    /// Statement root indices for this window.
    pub root_words: Vec<u32>,
}

/// Decoded semantic ProgramGraph payload from a `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectSemanticGraph {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Decoded semantic node rows.
    pub nodes: Vec<CAstSemanticPgNode>,
    /// Decoded semantic edge rows.
    pub edges: Vec<CAstSemanticPgEdge>,
    /// Number of semantic nodes carrying GNU builtin-specific roles.
    pub builtin_role_nodes: u64,
}

impl CObjectSemanticGraph {
    /// Iterate semantic nodes that carry GNU builtin-specific roles.
    pub fn builtin_nodes(&self) -> impl Iterator<Item = &CAstSemanticPgNode> {
        self.nodes.iter().filter(|node| node.has_builtin_role())
    }

    /// Iterate declaration/category nodes useful for symbol and type-index construction.
    pub fn declaration_nodes(&self) -> impl Iterator<Item = &CAstSemanticPgNode> {
        self.nodes
            .iter()
            .filter(|node| node.category == C_AST_PG_CATEGORY_DECLARATION)
    }

    /// Iterate control/category nodes useful for CFG and security sink/source traversals.
    pub fn control_nodes(&self) -> impl Iterator<Item = &CAstSemanticPgNode> {
        self.nodes
            .iter()
            .filter(|node| node.category == C_AST_PG_CATEGORY_CONTROL)
    }

    /// Iterate GNU-extension nodes, including inline asm, attributes, and builtin families.
    pub fn gnu_nodes(&self) -> impl Iterator<Item = &CAstSemanticPgNode> {
        self.nodes
            .iter()
            .filter(|node| node.category == C_AST_PG_CATEGORY_GNU)
    }

    /// Return the stable semantic role name for a node row.
    #[must_use]
    pub fn role_name(&self, node_index: usize) -> Option<&'static str> {
        self.nodes
            .get(node_index)
            .map(|node| c_ast_pg_role_name(node.role))
    }
}

/// Decoded `c_sema_scope` payload from a `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectSemaScope {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Decoded scope/declaration/identifier rows.
    pub records: Vec<CSemaScopeRecord>,
    /// Number of rows carrying declaration information.
    pub declaration_rows: u64,
    /// Number of rows carrying interned identifier ids.
    pub identifier_rows: u64,
}

/// Static-analysis symbol row derived from GPU `c_sema_scope` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectSymbolRef {
    /// Scope id visible at the declaration token.
    pub scope_id: u32,
    /// Parent scope id.
    pub parent_scope_id: u32,
    /// Numeric declaration kind.
    pub decl_kind: u32,
    /// Stable declaration kind name.
    pub decl_kind_name: &'static str,
    /// FNV-1a identifier hash emitted by the GPU interner.
    pub identifier_id: u32,
    /// Source byte offset for the identifier token.
    pub token_start: u32,
    /// Source byte length for the identifier token.
    pub token_len: u32,
}

impl CObjectSemaScope {
    /// Iterate semantic rows that introduce declarations.
    pub fn declarations(&self) -> impl Iterator<Item = &CSemaScopeRecord> {
        self.records
            .iter()
            .filter(|record| record.has_declaration())
    }

    /// Iterate semantic rows that carry interned identifier hashes and source spans.
    pub fn identifiers(&self) -> impl Iterator<Item = &CSemaScopeRecord> {
        self.records.iter().filter(|record| record.has_identifier())
    }

    /// Iterate declaration rows as symbol-oriented records with stable kind names.
    pub fn symbols(&self) -> impl Iterator<Item = CObjectSymbolRef> + '_ {
        self.declarations().map(|record| CObjectSymbolRef {
            scope_id: record.scope_id,
            parent_scope_id: record.parent_scope_id,
            decl_kind: record.decl_kind,
            decl_kind_name: record.decl_kind_name(),
            identifier_id: record.identifier_id,
            token_start: record.token_start,
            token_len: record.token_len,
        })
    }
}

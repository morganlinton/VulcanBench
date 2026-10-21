#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangDeclarationFact {
    /// clang AST node kind, such as `FunctionDecl`, `TypedefDecl`, or `FieldDecl`.
    pub(crate) kind: String,
    /// Declaration name when clang reports one.
    pub(crate) name: Option<String>,
    /// clang qualified type spelling when present on the declaration node.
    pub(crate) qual_type: Option<String>,
    /// Source file for the declaration location.
    pub(crate) file: String,
    /// One-based declaration line when clang reports one.
    pub(crate) line: Option<u32>,
    /// One-based declaration column when clang reports one.
    pub(crate) column: Option<u32>,
}

/// One clang statement/expression fact whose primary location belongs to the requested user file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangAstStructureFact {
    /// clang AST node kind, such as `ReturnStmt`, `BinaryOperator`, or `CallExpr`.
    pub(crate) kind: String,
    /// clang qualified type spelling when present on the statement/expression node.
    pub(crate) qual_type: Option<String>,
    /// Referenced declaration name when clang reports one.
    pub(crate) referenced_decl_name: Option<String>,
    /// Source file for the node location.
    pub(crate) file: String,
    /// One-based node line when clang reports one.
    pub(crate) line: Option<u32>,
    /// One-based node column when clang reports one.
    pub(crate) column: Option<u32>,
}

/// One clang type fact extracted from a declaration or expression node in the user file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangTypeFact {
    /// AST node kind that owns this type fact.
    pub(crate) owner_kind: String,
    /// AST node name when clang reports one.
    pub(crate) owner_name: Option<String>,
    /// clang qualified type spelling.
    pub(crate) qual_type: String,
    /// clang desugared qualified type spelling when present.
    pub(crate) desugared_qual_type: Option<String>,
    /// Whether the type spelling or node metadata indicates a typedef alias.
    pub(crate) uses_typedef: bool,
    /// Whether the type spelling uses `typeof`/`__typeof__`.
    pub(crate) uses_typeof: bool,
    /// Whether the type includes `const`.
    pub(crate) is_const: bool,
    /// Whether the type includes `volatile`.
    pub(crate) is_volatile: bool,
    /// Whether the type includes `restrict`.
    pub(crate) is_restrict: bool,
    /// Number of pointer stars in the qualified type spelling.
    pub(crate) pointer_depth: u32,
    /// Number of array extents in the qualified type spelling.
    pub(crate) array_depth: u32,
    /// Whether this is a function type spelling.
    pub(crate) is_function: bool,
    /// Tag kind detected from the qualified type spelling: `struct`, `union`, or `enum`.
    pub(crate) tag_kind: Option<String>,
    /// Source file for the owner node.
    pub(crate) file: String,
    /// One-based source line when clang reports one.
    pub(crate) line: Option<u32>,
    /// One-based source column when clang reports one.
    pub(crate) column: Option<u32>,
}

/// One clang symbol/scope fact for a declaration in the user file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangSymbolScopeFact {
    /// clang declaration node kind.
    pub(crate) kind: String,
    /// Declaration name.
    pub(crate) name: String,
    /// clang storage class when present.
    pub(crate) storage_class: Option<String>,
    /// clang previous declaration pointer when this declaration redeclares an earlier one.
    pub(crate) previous_decl: Option<String>,
    /// Owning declaration kind when this declaration is nested.
    pub(crate) owner_kind: Option<String>,
    /// Owning declaration name when this declaration is nested.
    pub(crate) owner_name: Option<String>,
    /// Inferred lexical scope kind.
    pub(crate) scope_kind: String,
    /// Inferred linkage class.
    pub(crate) linkage: String,
    /// Inferred visibility class.
    pub(crate) visibility: String,
    /// Source file for the declaration.
    pub(crate) file: String,
    /// One-based source line when clang reports one.
    pub(crate) line: Option<u32>,
    /// One-based source column when clang reports one.
    pub(crate) column: Option<u32>,
}

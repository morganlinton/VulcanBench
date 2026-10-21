//! Canonical node kinds for the Performance Graph (PG) layout.

/// Canonical node kinds for the Performance Graph (PG) layout.
/// Shared between frontend consumers and vyre as the definitive source of truth.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum PgNodeKind {
    /// Variable declaration node.
    VariableDecl = 1,
    /// Variable usage node.
    VariableUse = 2,
    /// Assignment statement node.
    Assignment = 3,
    /// Binary operation node.
    Binary = 4,
    /// Comparison operation node.
    Comparison = 5,
    /// Function call node.
    FunctionCall = 6,
    /// Function definition node.
    FunctionDef = 7,
    /// If statement node.
    IfStmt = 8,
    /// For loop node.
    ForStmt = 9,
    /// While loop node.
    WhileStmt = 10,
    /// Return statement node.
    ReturnStmt = 11,
    /// Pointer dereference node.
    Deref = 12,
    /// Address-of node.
    AddrOf = 13,
    /// Type cast node.
    Cast = 14,
    /// Member access node.
    MemberAccess = 15,
    /// Array access node.
    ArrayAccess = 16,
    /// Struct declaration node.
    StructDecl = 17,
    /// Integer literal node.
    LiteralInt = 18,
    /// String literal node.
    LiteralStr = 19,
    /// Floating point literal node.
    LiteralFloat = 20,
}

impl PgNodeKind {
    /// Converts a u32 into a `PgNodeKind` if it is valid.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::VariableDecl),
            2 => Some(Self::VariableUse),
            3 => Some(Self::Assignment),
            4 => Some(Self::Binary),
            5 => Some(Self::Comparison),
            6 => Some(Self::FunctionCall),
            7 => Some(Self::FunctionDef),
            8 => Some(Self::IfStmt),
            9 => Some(Self::ForStmt),
            10 => Some(Self::WhileStmt),
            11 => Some(Self::ReturnStmt),
            12 => Some(Self::Deref),
            13 => Some(Self::AddrOf),
            14 => Some(Self::Cast),
            15 => Some(Self::MemberAccess),
            16 => Some(Self::ArrayAccess),
            17 => Some(Self::StructDecl),
            18 => Some(Self::LiteralInt),
            19 => Some(Self::LiteralStr),
            20 => Some(Self::LiteralFloat),
            _ => None,
        }
    }
}

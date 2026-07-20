//! AST opcode constants shared by parsing optimizer primitives.

/// Variable alias node.
pub const AST_VAR: u32 = 1;
/// Integer constant node.
pub const AST_CONST_INT: u32 = 2;
/// Assignment node.
pub const AST_ASSIGN: u32 = 4;
/// Integer addition node.
pub const AST_ADD: u32 = 10;
/// Integer multiplication node.
pub const AST_MUL: u32 = 12;
/// Pointer dereference node.
pub const AST_PTR_DEREF: u32 = 27;

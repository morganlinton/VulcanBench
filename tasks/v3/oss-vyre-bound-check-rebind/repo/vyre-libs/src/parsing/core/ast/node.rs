//! Universal GPU AST Node OpCodes for all frontends.
//!
//! Primary AST Node Definitions
//!
//! Exposes all 16-bit intermediate OpCodes representing the abstract structural tree.

/// Primitive constant number node
pub const AST_CONST_INT: u32 = 1;
/// Variable reference
pub const AST_VAR: u32 = 2;
/// Binary Addition
pub const AST_ADD: u32 = 3;
/// Binary Subtraction
pub const AST_SUB: u32 = 4;
/// Binary Multiplication
pub const AST_MUL: u32 = 5;
/// Binary Division
pub const AST_DIV: u32 = 6;
/// Variable Mutation/Assignment
pub const AST_ASSIGN: u32 = 7;
/// Function dispatch
pub const AST_CALL: u32 = 8;
/// Output return
pub const AST_RET: u32 = 9;
/// Conditional jump
pub const AST_IF: u32 = 10;
/// Dereference Operator
pub const AST_PTR_DEREF: u32 = 11;
/// Type demotion/promotion Cast
pub const AST_CAST: u32 = 12;
/// Remainder
pub const AST_MOD: u32 = 13;
/// Equality comparison
pub const AST_EQ: u32 = 14;
/// Inequality comparison
pub const AST_NE: u32 = 15;
/// Less-than comparison
pub const AST_LT: u32 = 16;
/// Greater-than comparison
pub const AST_GT: u32 = 17;
/// Less-than-or-equal comparison
pub const AST_LE: u32 = 18;
/// Greater-than-or-equal comparison
pub const AST_GE: u32 = 19;
/// Logical conjunction
pub const AST_LOGICAL_AND: u32 = 20;
/// Logical disjunction
pub const AST_LOGICAL_OR: u32 = 21;

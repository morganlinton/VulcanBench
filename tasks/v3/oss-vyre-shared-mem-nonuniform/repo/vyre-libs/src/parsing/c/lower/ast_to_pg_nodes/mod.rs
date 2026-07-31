//! AST-to-ProgramGraph lowering.
//!
//! Module layout:
//! - `mod.rs`: shared C_AST_PG constants and public exports.
//! - `gpu_program.rs`: dispatchable GPU IR builders.
//! - `reference.rs`: explicit CPU oracle functions used by parity and fixtures.
//! - `semantic.rs`: semantic role/category helpers shared by GPU and oracle code.
//! - `witness.rs`: harness fixtures.

mod gpu_program;
#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
mod semantic;
#[cfg(any(test, feature = "cpu-parity"))]
mod witness;

pub use gpu_program::{
    c_lower_ast_to_pg_nodes, c_lower_ast_to_pg_semantic_graph,
    c_lower_ast_to_pg_semantic_graph_with_pg,
    c_lower_ast_to_pg_semantic_graph_with_pg_no_control_resolution, PgReferenceDecodeError,
    SemanticPgReference,
};
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::{
    reference_ast_to_pg_nodes, reference_ast_to_pg_semantic_graph, try_reference_ast_to_pg_nodes,
    try_reference_ast_to_pg_semantic_graph,
};

// Sibling re-exports keep the GPU builder, oracle, and witness modules on one
// explicit helper surface. If a helper becomes pass-specific, move it into
// that pass instead of growing this shared prelude.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::harness::OpEntry;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre::ir::Expr;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::predicate::node_kind;

/// Number of `u32` words in one packed VAST node.
const VAST_NODE_STRIDE_U32: u32 = 10;
/// Number of `u32` words in one packed `PgNode`.
const PG_NODE_STRIDE_U32: u32 = 6;
/// Number of `u32` words in one semantic PG node witness row.
pub const C_AST_PG_SEMANTIC_NODE_STRIDE_U32: u32 = 10;
/// Number of `u32` words in one semantic PG edge witness row.
pub const C_AST_PG_EDGE_STRIDE_U32: u32 = 6;
/// Number of edge witness rows emitted per AST node.
pub const C_AST_PG_EDGE_ROWS_PER_NODE: u32 = 5;

/// No C AST semantic category was assigned.
pub const C_AST_PG_CATEGORY_NONE: u32 = 0;
/// Control-flow and label-like C AST node.
pub const C_AST_PG_CATEGORY_CONTROL: u32 = 1;
/// Expression, initializer, designator, or statement-expression node.
pub const C_AST_PG_CATEGORY_EXPRESSION: u32 = 2;
/// GNU extension node such as inline asm, attributes, or GNU builtins.
pub const C_AST_PG_CATEGORY_GNU: u32 = 3;
/// Declaration, declarator, type, or function-definition node.
pub const C_AST_PG_CATEGORY_DECLARATION: u32 = 4;

/// No specialized C AST role was assigned.
pub const C_AST_PG_ROLE_NONE: u32 = 0;
/// C/GNU label definition.
pub const C_AST_PG_ROLE_LABEL: u32 = 1;
/// `case` label.
pub const C_AST_PG_ROLE_CASE: u32 = 2;
/// `default` label.
pub const C_AST_PG_ROLE_DEFAULT: u32 = 3;
/// GNU statement expression `({ ... })`.
pub const C_AST_PG_ROLE_STATEMENT_EXPR: u32 = 4;
/// GNU inline asm statement or declarator asm suffix.
pub const C_AST_PG_ROLE_INLINE_ASM: u32 = 5;
/// GNU asm template string.
pub const C_AST_PG_ROLE_ASM_TEMPLATE: u32 = 6;
/// GNU asm output operand.
pub const C_AST_PG_ROLE_ASM_OUTPUT: u32 = 7;
/// GNU asm input operand.
pub const C_AST_PG_ROLE_ASM_INPUT: u32 = 8;
/// GNU asm clobber string.
pub const C_AST_PG_ROLE_ASM_CLOBBER: u32 = 9;
/// GNU asm-goto label operand.
pub const C_AST_PG_ROLE_ASM_GOTO_LABEL: u32 = 10;
/// GNU asm qualifier such as `volatile` or `goto`.
pub const C_AST_PG_ROLE_ASM_QUALIFIER: u32 = 11;
/// GNU `__attribute__` wrapper.
pub const C_AST_PG_ROLE_GNU_ATTRIBUTE: u32 = 12;
/// Specific GNU attribute payload such as `section`, `weak`, or `aligned`.
pub const C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL: u32 = 13;
/// C initializer-list brace.
pub const C_AST_PG_ROLE_INITIALIZER_LIST: u32 = 14;
/// Field designator or member-access operator.
pub const C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS: u32 = 15;
/// Array designator or subscript operator.
pub const C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT: u32 = 16;
/// GNU range designator ellipsis.
pub const C_AST_PG_ROLE_RANGE_DESIGNATOR: u32 = 17;
/// Assignment-expression node, including designator assignment witnesses.
pub const C_AST_PG_ROLE_ASSIGNMENT: u32 = 18;
/// Function definition declarator identifier.
pub const C_AST_PG_ROLE_FUNCTION_DEFINITION: u32 = 19;
/// Function declarator parameter-list node.
pub const C_AST_PG_ROLE_FUNCTION_DECLARATOR: u32 = 20;
/// Aggregate declaration/specifier node.
pub const C_AST_PG_ROLE_AGGREGATE_DECL: u32 = 21;
/// Field declarator identifier node.
pub const C_AST_PG_ROLE_FIELD_DECL: u32 = 22;
/// Typedef declarator node.
pub const C_AST_PG_ROLE_TYPEDEF_DECL: u32 = 23;
/// Enumerator declarator node.
pub const C_AST_PG_ROLE_ENUMERATOR_DECL: u32 = 24;
/// Pointer declarator node.
pub const C_AST_PG_ROLE_POINTER_DECL: u32 = 25;
/// Array declarator node.
pub const C_AST_PG_ROLE_ARRAY_DECL: u32 = 26;
/// Bit-field declarator node.
pub const C_AST_PG_ROLE_BIT_FIELD_DECL: u32 = 27;
/// `_Static_assert` declaration node.
pub const C_AST_PG_ROLE_STATIC_ASSERT_DECL: u32 = 28;
/// Generic expression operator or builtin expression witness.
pub const C_AST_PG_ROLE_EXPRESSION: u32 = 29;
/// Generic declaration witness from the shared predicate node-kind set.
pub const C_AST_PG_ROLE_DECLARATION: u32 = 30;
/// `goto` branch statement.
pub const C_AST_PG_ROLE_GOTO: u32 = 31;
/// `switch` selection statement.
pub const C_AST_PG_ROLE_SWITCH: u32 = 32;
/// `if` or `else` selection statement.
pub const C_AST_PG_ROLE_SELECTION: u32 = 33;
/// `for`, `while`, or `do` loop statement.
pub const C_AST_PG_ROLE_LOOP: u32 = 34;
/// `return` statement.
pub const C_AST_PG_ROLE_RETURN: u32 = 35;
/// `break` statement.
pub const C_AST_PG_ROLE_BREAK: u32 = 36;
/// `continue` statement.
pub const C_AST_PG_ROLE_CONTINUE: u32 = 37;
/// `__builtin_unreachable` terminator statement.
pub const C_AST_PG_ROLE_UNREACHABLE: u32 = 38;
/// `_Alignof` expression.
pub const C_AST_PG_ROLE_ALIGNOF: u32 = 39;
/// Pointer declarator participating in function-pointer declarator shape.
pub const C_AST_PG_ROLE_FUNCTION_POINTER_DECL: u32 = 40;

/// No semantic edge exists in this witness row.
pub const C_AST_PG_EDGE_NONE: u32 = 0;
/// Parent contains child.
pub const C_AST_PG_EDGE_PARENT: u32 = 1;
/// Node points to first child.
pub const C_AST_PG_EDGE_FIRST_CHILD: u32 = 2;
/// Node points to next sibling.
pub const C_AST_PG_EDGE_NEXT_SIBLING: u32 = 3;
/// `goto` statement points to the resolved label statement in the same root body.
pub const C_AST_PG_EDGE_GOTO_TARGET: u32 = 4;
/// `switch` statement points to the first selector expression node.
pub const C_AST_PG_EDGE_SWITCH_SELECTOR: u32 = 5;
/// Enclosing `switch` statement points to a `case` label.
pub const C_AST_PG_EDGE_SWITCH_CASE: u32 = 6;
/// Enclosing `switch` statement points to a `default` label.
pub const C_AST_PG_EDGE_SWITCH_DEFAULT: u32 = 7;
/// `case` label points to the first node of its value expression.
pub const C_AST_PG_EDGE_CASE_VALUE: u32 = 8;

const IDX_KIND: usize = 0;
const IDX_PARENT: usize = 1;
const IDX_FIRST_CHILD: usize = 2;
const IDX_NEXT_SIBLING: usize = 3;
const IDX_SRC_BYTE_OFF: usize = 5;
const IDX_SRC_BYTE_LEN: usize = 6;
const IDX_ATTR_OFF: usize = 7;
const IDX_ATTR_LEN: usize = 8;
const IDX_RESERVED: usize = 9;

const OP_ID: &str = "vyre-libs::parsing::c::lower::ast_to_pg_nodes";
const SEMANTIC_OP_ID: &str = "vyre-libs::parsing::c::lower::ast_to_pg_semantic_graph";

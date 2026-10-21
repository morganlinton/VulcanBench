use std::collections::HashSet;
use std::path::Path;

mod abi;
mod ast;
mod common;
mod model;
mod sema_scope;
mod semantic_graph;
#[cfg(test)]
mod tests;

pub use abi::{
    decode_object_abi_layout, decode_object_abi_layout_file, CObjectAbiLayout,
    CObjectAbiLayoutEntry,
};
pub use ast::{decode_object_ast, decode_object_ast_file};
pub use model::{
    CAstSemanticPgEdge, CAstSemanticPgNode, CObjectAst, CObjectAstWindow, CObjectSemaScope,
    CObjectSemanticGraph, CObjectSymbolRef, CSemaScopeRecord,
};
pub use sema_scope::{decode_object_sema_scope, decode_object_sema_scope_file};
pub use semantic_graph::{decode_object_semantic_graph, decode_object_semantic_graph_file};

pub(crate) use abi::decode_object_abi_layout_from_container;
pub(crate) use ast::decode_object_ast_from_container;
pub(crate) use sema_scope::decode_object_sema_scope_from_container;
pub(crate) use semantic_graph::decode_object_semantic_graph_from_container;

use common::{checked_count_u64, decode_u32_words};
use model::{is_known_decl_kind, is_known_semantic_category, is_known_semantic_role};
use vyre_libs::parsing::c::lower::{
    C_AST_PG_CATEGORY_CONTROL, C_AST_PG_CATEGORY_DECLARATION, C_AST_PG_CATEGORY_EXPRESSION,
    C_AST_PG_CATEGORY_GNU, C_AST_PG_CATEGORY_NONE, C_AST_PG_EDGE_STRIDE_U32,
    C_AST_PG_ROLE_AGGREGATE_DECL, C_AST_PG_ROLE_ALIGNOF, C_AST_PG_ROLE_ARRAY_DECL,
    C_AST_PG_ROLE_ARRAY_DESIGNATOR_OR_SUBSCRIPT, C_AST_PG_ROLE_ASM_CLOBBER,
    C_AST_PG_ROLE_ASM_GOTO_LABEL, C_AST_PG_ROLE_ASM_INPUT, C_AST_PG_ROLE_ASM_OUTPUT,
    C_AST_PG_ROLE_ASM_QUALIFIER, C_AST_PG_ROLE_ASM_TEMPLATE, C_AST_PG_ROLE_ASSIGNMENT,
    C_AST_PG_ROLE_BIT_FIELD_DECL, C_AST_PG_ROLE_BREAK, C_AST_PG_ROLE_CASE, C_AST_PG_ROLE_CONTINUE,
    C_AST_PG_ROLE_DECLARATION, C_AST_PG_ROLE_DEFAULT, C_AST_PG_ROLE_ENUMERATOR_DECL,
    C_AST_PG_ROLE_EXPRESSION, C_AST_PG_ROLE_FIELD_DECL,
    C_AST_PG_ROLE_FIELD_DESIGNATOR_OR_MEMBER_ACCESS, C_AST_PG_ROLE_FUNCTION_DECLARATOR,
    C_AST_PG_ROLE_FUNCTION_DEFINITION, C_AST_PG_ROLE_FUNCTION_POINTER_DECL,
    C_AST_PG_ROLE_GNU_ATTRIBUTE, C_AST_PG_ROLE_GNU_ATTRIBUTE_DETAIL, C_AST_PG_ROLE_GOTO,
    C_AST_PG_ROLE_INITIALIZER_LIST, C_AST_PG_ROLE_INLINE_ASM, C_AST_PG_ROLE_LABEL,
    C_AST_PG_ROLE_LOOP, C_AST_PG_ROLE_NONE, C_AST_PG_ROLE_POINTER_DECL,
    C_AST_PG_ROLE_RANGE_DESIGNATOR, C_AST_PG_ROLE_RETURN, C_AST_PG_ROLE_SELECTION,
    C_AST_PG_ROLE_STATEMENT_EXPR, C_AST_PG_ROLE_STATIC_ASSERT_DECL, C_AST_PG_ROLE_SWITCH,
    C_AST_PG_ROLE_TYPEDEF_DECL, C_AST_PG_ROLE_UNREACHABLE, C_AST_PG_SEMANTIC_NODE_STRIDE_U32,
};
use vyre_libs::parsing::c::sema::lookup::{
    DECL_KIND_ENUM_CONSTANT, DECL_KIND_FUNCTION, DECL_KIND_FUNCTION_DECL, DECL_KIND_LABEL,
    DECL_KIND_NONE, DECL_KIND_TYPEDEF, DECL_KIND_VARIABLE,
};

use crate::api::object_io::{decode_embedded_object, read_object_file};
use crate::object_format::{SectionTag, Vyrecob2};

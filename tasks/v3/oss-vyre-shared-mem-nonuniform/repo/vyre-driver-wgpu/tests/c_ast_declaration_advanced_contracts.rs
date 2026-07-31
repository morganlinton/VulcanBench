//! Advanced C declaration and declarator contracts.
//!
//! Coverage gaps filled:
//!   * deeply nested struct / union / enum definitions
//!   * anonymous struct/union members
//!   * typedefs with multiple complex declarators (struct tag + pointer)
//!   * triple-star pointers with interleaved qualifiers
//!   * storage-class combinations: _Thread_local, _Atomic, register, inline
//!   * bit-fields inside nested structs
//!   * GNU attributes on struct fields and typedef declarations
//!   * pointer-to-function-pointer declarators
//!   * arrays of function pointers with qualified parameters
//!
//! Every test asserts CPU/GPU parity and meaningful AST/VAST/PG invariants.
//! A missing GPU adapter is a configuration failure, never a skip.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
#[allow(dead_code)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, assert_pg_preserves_row, c_fixture, kind_at, row_indices,
    run_gpu_pg_lower_with_count as run_gpu_pg_lower, word_at, Fixture, VAST_STRIDE_U32,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_BIT_FIELD_DECL,
    C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FIELD_DECL,
    C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_POINTER_DECL,
    C_AST_KIND_STRUCT_DECL, C_AST_KIND_TYPEDEF_DECL, C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

const TYPEDEF_FLAGS_FIELD: usize = 7;
const TYPEDEF_FLAG_DECL: u32 = 1 << 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn flags_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + TYPEDEF_FLAGS_FIELD)
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    (vast.len() / (VAST_STRIDE_U32 * 4)) as u32
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// struct outer {
///     union {
///         struct { int x; } s;
///         int y;
///     } u;
///     enum { A = 1, B = 2 } e;
/// };
/// ```
fn fixture_nested_struct_union_enum() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("outer", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("union", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("struct", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("s", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("u", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("enum", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("A", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("1", TOK_INTEGER),
        (",", TOK_COMMA),
        ("B", TOK_IDENTIFIER),
        ("=", TOK_ASSIGN),
        ("2", TOK_INTEGER),
        ("}", TOK_RBRACE),
        ("e", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// struct {
///     union {
///         int i;
///         float f;
///     };
///     int tag;
/// };
/// ```
fn fixture_anonymous_struct_union() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("union", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("i", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("float", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
        ("int", TOK_IDENTIFIER),
        ("tag", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// typedef struct Node { int v; } Node, *NodePtr;
/// ```
fn fixture_typedef_multiple_declarators() -> Fixture {
    c_fixture![
        ("typedef", TOK_IDENTIFIER),
        ("struct", TOK_IDENTIFIER),
        ("Node", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("v", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("Node", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("*", TOK_STAR),
        ("NodePtr", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// const int * const * volatile * restrict p;
/// ```
fn fixture_deeply_nested_pointer() -> Fixture {
    c_fixture![
        ("const", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("const", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("volatile", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("restrict", TOK_IDENTIFIER),
        ("p", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// static inline int f(void);
/// extern register int x;
/// _Thread_local _Atomic int y;
/// ```
fn fixture_storage_class_combinations() -> Fixture {
    c_fixture![
        ("static", TOK_IDENTIFIER),
        ("inline", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("extern", TOK_IDENTIFIER),
        ("register", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("_Thread_local", TOK_IDENTIFIER),
        ("_Atomic", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("y", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// struct {
///     unsigned int a : 4;
///     struct {
///         int b : 8;
///         unsigned int : 0;
///     } inner;
/// };
/// ```
fn fixture_bitfield_nested_struct() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("unsigned", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("a", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("4", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("struct", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("int", TOK_IDENTIFIER),
        ("b", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("8", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("unsigned", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        (":", TOK_COLON),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        ("inner", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// struct {
///     __attribute__((aligned(8))) int x;
/// };
/// typedef int __attribute__((packed)) packed_int;
/// ```
fn fixture_gnu_attribute_field_and_typedef() -> Fixture {
    c_fixture![
        ("struct", TOK_IDENTIFIER),
        ("{", TOK_LBRACE),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("aligned", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("8", TOK_INTEGER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("int", TOK_IDENTIFIER),
        ("x", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
        ("typedef", TOK_IDENTIFIER),
        ("int", TOK_IDENTIFIER),
        ("__attribute__", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("packed", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("packed_int", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// int (**fp)(void);
/// ```
fn fixture_function_pointer_to_pointer() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("*", TOK_STAR),
        ("fp", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("void", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

/// ```c
/// int (*handlers[4])(int, const char * restrict);
/// ```
fn fixture_array_of_function_pointers_qualified() -> Fixture {
    c_fixture![
        ("int", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("handlers", TOK_IDENTIFIER),
        ("[", TOK_LBRACKET),
        ("4", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("int", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        ("const", TOK_IDENTIFIER),
        ("char", TOK_IDENTIFIER),
        ("*", TOK_STAR),
        ("restrict", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ]
}

// ---------------------------------------------------------------------------
// CPU reference contracts  -  nested struct/union/enum
// ---------------------------------------------------------------------------

mod c_ast_declaration_advanced_contracts_part1 {

    include!("__split/c_ast_declaration_advanced_contracts_part1.rs");
}
mod c_ast_declaration_advanced_contracts_part2 {
    include!("__split/c_ast_declaration_advanced_contracts_part2.rs");
}

//! End-to-end C AST tests for initializer lists, designators, compound literals,
//! and aggregate lowering (array / struct / union / enum) with GPU/CPU parity.
//!
//! Coverage:
//!   * plain initializer lists for arrays and structs
//!   * nested designated initializers (dot and array subscript mixed)
//!   * compound literals in assignment and call contexts
//!   * union designated initializers
//!   * enum declarations with explicit/implicit values
//!   * GNU range designators (`[a ... b]`)
//!   * PG lowering preservation (kind, span, parent, first_child, next_sibling)

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds,
    C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR,
    C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_FIELD_DECL, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_MEMBER_ACCESS_EXPR,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{run_gpu_pg_lower, starts_for_lens};

#[path = "support/c_ast_initializer_designator.rs"]
mod c_ast_initializer_designator_support;
use c_ast_initializer_designator_support::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// int arr[3] = {1, 2, 3};
/// ```
fn fixture_array_initializer_list() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct Point p = {10, "label"};
/// ```
fn fixture_struct_initializer_list() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_STRING,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 5, 1, 1, 1, 2, 1, 7, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// union U u = {.i = 42};
/// ```
fn fixture_union_designated_init() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_UNION,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![5, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// enum Color { RED = 0, GREEN, BLUE = 2 };
/// enum Color c = GREEN;
/// ```
fn fixture_enum_with_initializer() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_ENUM,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_ENUM,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![4, 5, 1, 3, 1, 1, 1, 5, 1, 4, 1, 1, 1, 1, 4, 5, 1, 1, 5, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct config cfg = {
///   .name = "test",
///   .dims = { [0] = 1, [1] = { .x = 2, .y = 3 } },
///   .flags[2] = 1,
/// };
/// ```
fn fixture_nested_designator_mixed() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_RBRACE,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct Rect r = (struct Rect){ .w = 10, .h = 20 };
/// ```
fn fixture_compound_literal_expr() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 4, 1, 1, 1, 6, 4, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 2, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// void f(struct S);
/// f((struct S){ .a = 1 });
/// ```
fn fixture_compound_literal_in_call() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_VOID,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        4, 1, 1, 6, 1, 1, 1, 1, 1, 1, 6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// CPU reference tests  -  shape & kind correctness
// ---------------------------------------------------------------------------

mod c_ast_initializer_designator_e2e_part1 {

    include!("__split/c_ast_initializer_designator_e2e_part1.rs");
}
mod c_ast_initializer_designator_e2e_part2 {
    include!("__split/c_ast_initializer_designator_e2e_part2.rs");
}

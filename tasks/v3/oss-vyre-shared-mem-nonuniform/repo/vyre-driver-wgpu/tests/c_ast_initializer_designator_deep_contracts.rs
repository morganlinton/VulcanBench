//! Deep contracts for C AST initializer designators, compound literals,
//! assignment suppression/classification, string initializers, and full
//! CPU / GPU / PG parity.
//!
//! Coverage:
//!   * nested designators (field → array, field → struct)
//!   * GNU range designators `[a ... b]`
//!   * field designators in unions and structs
//!   * mixed positional / designated initializers
//!   * compound literals inside initializer lists
//!   * declaration initializer assignment suppression
//!   * designator assignment classification
//!   * string / char array initialization in nested aggregates
//!   * CPU reference, PG lowering preservation, GPU parity for all of the above

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL,
    C_AST_KIND_ARRAY_SUBSCRIPT_EXPR, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR,
    C_AST_KIND_FIELD_DECL, C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_RANGE_DESIGNATOR_EXPR, C_AST_KIND_STRUCT_DECL, C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{
    run_gpu_classifier, run_gpu_pg_lower, run_gpu_vast_builder_from_parts as run_gpu_vast_builder,
    starts_for_lens,
};

#[path = "support/c_ast_initializer_designator.rs"]
mod c_ast_initializer_designator_support;
use c_ast_initializer_designator_support::*;

fn assert_full_pipeline_parity(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    label: &str,
) {
    let raw_cpu = reference_c11_build_vast_nodes(tok_types, tok_starts, tok_lens);
    let raw_gpu = run_gpu_vast_builder(tok_types, tok_starts, tok_lens);
    assert_eq!(
        raw_gpu, raw_cpu,
        "{label}: GPU VAST builder must match CPU oracle"
    );

    let typed_cpu = reference_c11_classify_vast_node_kinds(&raw_cpu);
    let typed_gpu = run_gpu_classifier(&raw_cpu);
    assert_eq!(
        typed_gpu, typed_cpu,
        "{label}: GPU classifier must match CPU oracle"
    );

    let pg_cpu = reference_ast_to_pg_nodes(&typed_cpu);
    let pg_gpu = run_gpu_pg_lower(&typed_cpu);
    assert_eq!(
        pg_gpu, pg_cpu,
        "{label}: GPU PG lowerer must match CPU oracle"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// ```c
/// struct S s = { .a[0] = 1, .b = { .c = 2 } };
/// ```
fn fixture_nested_field_array_designator() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int arr[10] = { [0 ... 3] = 1, [5] = 2 };
/// ```
fn fixture_range_designator_array() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_ELLIPSIS,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        3, 3, 1, 2, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// union U u = { .f = 42 };
/// ```
fn fixture_union_field_designator() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
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
/// struct S s = { 1, .b = 2, 3 };
/// ```
fn fixture_mixed_positional_designated() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct T t = { .inner = (struct S){ .x = 1 } };
/// ```
fn fixture_compound_literal_nested() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
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
        TOK_RBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 1, 1, 1, 1, 1, 5, 1, 1, 6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// int x = {1};
/// ```
fn fixture_assignment_suppression() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_INTEGER,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![3, 1, 1, 1, 1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct S s = { .a = 1 };
/// ```
fn fixture_designator_assignment_class() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
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
    let tok_lens = vec![6, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct Buf { char data[4]; } b = { .data = "abc" };
/// ```
fn fixture_string_char_array_nested() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 3, 1, 4, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 4, 1, 5, 1, 1];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// CPU reference contracts
// ---------------------------------------------------------------------------

mod c_ast_initializer_designator_deep_contracts_part1 {

    include!("__split/c_ast_initializer_designator_deep_contracts_part1.rs");
}
mod c_ast_initializer_designator_deep_contracts_part2 {
    include!("__split/c_ast_initializer_designator_deep_contracts_part2.rs");
}

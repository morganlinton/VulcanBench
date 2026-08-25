//! GPU/CPU parity end-to-end tests for deeply nested initializer lists
//! and designated initializers that appear in Linux-grade aggregate
//! declarations.
//!
//! Constructs under test:
//!   - deeply nested struct/array initializer lists with mixed designators
//!   - compound literals inside nested initializers
//!   - nested array designated initializers
//!   - deep designated initializers (chained member access)
//!
//! A missing GPU adapter is a configuration failure.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, Fixture, FixtureToken,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_MEMBER_ACCESS_EXPR,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

/// struct { int a[2]; struct { int b; } s; } x = { {1, 2}, {3} };
fn fixture_deeply_nested_initializer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// int a[2][2] = { [0] = {1, 2}, [1] = {3, 4} };
fn fixture_nested_array_designated_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

/// struct T t = { .s.b = 1 };
fn fixture_deep_designated_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("T", TOK_IDENTIFIER),
        FixtureToken::new("t", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_initializer_gpu_cpu_parity() {
    let fix = fixture_deeply_nested_initializer();
    assert_full_pipeline_parity(&fix, "deeply_nested_initializer");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let init_lists = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        init_lists.len() >= 3,
        "must contain outer and two inner initializer lists"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&17),
        "x must classify as VARIABLE"
    );
}

#[test]
fn nested_array_designated_init_gpu_cpu_parity() {
    let fix = fixture_nested_array_designated_init();
    assert_full_pipeline_parity(&fix, "nested_array_designated_init");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    let init_lists = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        init_lists.len() >= 3,
        "must contain outer and two inner initializer lists"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL).contains(&2),
        "a[2] must classify as ARRAY_DECL"
    );
}

#[test]
fn deep_designated_init_gpu_cpu_parity() {
    let fix = fixture_deep_designated_init();
    assert_full_pipeline_parity(&fix, "deep_designated_init");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);

    assert!(
        row_indices(&typed, C_AST_KIND_INITIALIZER_LIST).contains(&4),
        "deep designated init must contain an initializer list"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR).contains(&5),
        ".s must classify as MEMBER_ACCESS_EXPR"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR).contains(&7),
        ".b must classify as MEMBER_ACCESS_EXPR"
    );
}

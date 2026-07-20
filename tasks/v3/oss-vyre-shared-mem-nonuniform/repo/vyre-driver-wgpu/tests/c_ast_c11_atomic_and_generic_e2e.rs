//! GPU/CPU parity end-to-end tests for C11 _Atomic, _Generic, and typeof
//! combinations that appear in Linux-grade code but lack dedicated coverage.
//!
//! Constructs under test:
//!   - `_Atomic` as type specifier / qualifier in declarations and parameters
//!   - `_Atomic` mixed with pointer declarators
//!   - `_Generic` selection with multiple associations and default
//!   - `_Generic` nested inside call arguments
//!   - `typeof` combined with `_Atomic` in complex declarators
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
    reference_c11_classify_vast_node_kinds, C_AST_KIND_FIELD_DECL, C_AST_KIND_FUNCTION_DEFINITION,
    C_AST_KIND_GENERIC_SELECTION_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_STRUCT_DECL,
};
use vyre_primitives::predicate::node_kind;

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn fixture_atomic_variable() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("counter", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_atomic_pointer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_atomic_struct_member() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("s", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("val", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_atomic_parameter() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_generic_selection() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("_Generic", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("default", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_generic_in_call() -> Fixture {
    build_fixture(&[
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("_Generic", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("default", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_typeof_atomic_combo() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("_Atomic", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("q", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn atomic_variable_declaration_gpu_cpu_parity() {
    let fix = fixture_atomic_variable();
    assert_eq!(
        fix.tok_types[0], TOK_ATOMIC,
        "_Atomic must promote to keyword"
    );
    assert_full_pipeline_parity(&fix, "atomic_variable");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&2),
        "counter must classify as VARIABLE"
    );
}

#[test]
fn atomic_pointer_declaration_gpu_cpu_parity() {
    let fix = fixture_atomic_pointer();
    assert_eq!(fix.tok_types[0], TOK_ATOMIC);
    assert_full_pipeline_parity(&fix, "atomic_pointer");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&3),
        "pointer declarator must be present"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&4),
        "p must classify as VARIABLE"
    );
}

#[test]
fn atomic_struct_member_gpu_cpu_parity() {
    let fix = fixture_atomic_struct_member();
    assert_eq!(fix.tok_types[3], TOK_ATOMIC);
    assert_full_pipeline_parity(&fix, "atomic_struct_member");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, C_AST_KIND_STRUCT_DECL).contains(&0),
        "struct must classify as STRUCT_DECL"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_FIELD_DECL).contains(&5),
        "atomic struct member must classify as FIELD_DECL"
    );
}

#[test]
fn atomic_parameter_declaration_gpu_cpu_parity() {
    let fix = fixture_atomic_parameter();
    assert_eq!(fix.tok_types[3], TOK_ATOMIC);
    assert_full_pipeline_parity(&fix, "atomic_parameter");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION).contains(&1)
            || row_indices(&typed, node_kind::FUNCTION_DECL).contains(&1),
        "f with a body must classify as FUNCTION_DEFINITION or FUNCTION_DECL"
    );
}

#[test]
fn generic_selection_expression_gpu_cpu_parity() {
    let fix = fixture_generic_selection();
    assert_eq!(
        fix.tok_types[3], TOK_GENERIC,
        "_Generic must promote to keyword"
    );
    assert_full_pipeline_parity(&fix, "generic_selection");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GENERIC_SELECTION_EXPR),
        vec![3],
        "_Generic must classify as GENERIC_SELECTION_EXPR"
    );
}

#[test]
fn generic_in_call_argument_gpu_cpu_parity() {
    let fix = fixture_generic_in_call();
    assert_eq!(fix.tok_types[2], TOK_GENERIC);
    assert_full_pipeline_parity(&fix, "generic_in_call");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GENERIC_SELECTION_EXPR),
        vec![2],
        "_Generic inside call must classify as GENERIC_SELECTION_EXPR"
    );
}

#[test]
fn typeof_atomic_combination_gpu_cpu_parity() {
    let fix = fixture_typeof_atomic_combo();
    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF, "typeof must promote");
    assert_eq!(
        fix.tok_types[2], TOK_ATOMIC,
        "_Atomic inside typeof must promote"
    );
    assert_full_pipeline_parity(&fix, "typeof_atomic_combo");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&5),
        "pointer declarator must be present after typeof(_Atomic int)"
    );
}

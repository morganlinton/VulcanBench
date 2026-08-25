//! Contracts for GNU/C23 `typeof_unqual` declarators without token spoofing.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{
    assert_full_pipeline_parity, build_fixture, row_indices, Fixture, FixtureToken,
};
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_POINTER_DECL, C_AST_KIND_SIZEOF_EXPR,
};
use vyre_primitives::predicate::node_kind;

fn real_typeof_unqual_function_pointer_table() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__typeof_unqual__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("const", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("table", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn typedef_over_typeof_unqual_pointer() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("typeof_unqual", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("alias_t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("alias_t", TOK_IDENTIFIER),
        FixtureToken::new("value", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn typed_rows(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

#[test]
fn real_typeof_unqual_drives_complex_declarator_shape() {
    let fix = real_typeof_unqual_function_pointer_table();
    assert_eq!(
        fix.tok_types[0], TOK_GNU_TYPEOF_UNQUAL,
        "real __typeof_unqual__ spelling must promote through the keyword pass"
    );
    assert_full_pipeline_parity(&fix, "real_typeof_unqual_function_pointer_table");

    let typed = typed_rows(&fix);
    assert!(
        row_indices(&typed, C_AST_KIND_SIZEOF_EXPR).contains(&0),
        "__typeof_unqual__ must classify as the typeof operator row"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![4, 7],
        "both result pointer and table element pointer must be declarator rows"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL).contains(&9),
        "table[2] must remain an array declarator"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR).contains(&13),
        "function-pointer table suffix must classify as function declarator"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&8),
        "table identifier must remain the declarator variable"
    );
}

#[test]
fn typedef_over_typeof_unqual_is_visible_to_later_declarators() {
    let fix = typedef_over_typeof_unqual_pointer();
    assert_full_pipeline_parity(&fix, "typedef_over_typeof_unqual_pointer");

    let typed = typed_rows(&fix);
    assert!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL).contains(&5),
        "typedef typeof_unqual(int) *alias_t must classify pointer declarator"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&9),
        "alias_t value must classify value as a typedef-backed declarator"
    );
}

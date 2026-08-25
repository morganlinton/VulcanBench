//! GPU/CPU parity tests for declaration container VAST kinds.
//!
//! Linux-grade C depends on aggregate tag declarations, typedefs, function
//! definitions, bitfields, and `_Static_assert` being semantic AST rows rather
//! than raw keyword noise.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_BIT_FIELD_DECL, C_AST_KIND_ENUM_DECL,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_STATIC_ASSERT_DECL, C_AST_KIND_STRUCT_DECL,
    C_AST_KIND_TYPEDEF_DECL, C_AST_KIND_UNION_DECL,
};
use vyre_primitives::predicate::node_kind;

const VAST_STRIDE_U32: usize = 10;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::run_gpu_classifier_with_count;

mod common;
use common::c_fixture::*;

fn row_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let expected = reference_c11_classify_vast_node_kinds(&annotated);
    let actual =
        run_gpu_classifier_with_count(&annotated, (annotated.len() / (VAST_STRIDE_U32 * 4)) as u32);
    assert_eq!(
        actual, expected,
        "GPU C VAST classifier must match the CPU oracle"
    );
    expected
}

fn aggregate_fixture() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("foo", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("flags", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("union", TOK_IDENTIFIER),
        FixtureToken::new("cell", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("raw", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("enum", TOK_IDENTIFIER),
        FixtureToken::new("mode", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("MODE_A", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("MODE_B", TOK_IDENTIFIER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn aggregate_containers_and_bitfields_are_semantic_rows() {
    let typed = classify(&aggregate_fixture());
    assert_eq!(row_indices(&typed, C_AST_KIND_STRUCT_DECL), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_BIT_FIELD_DECL), vec![7]);
    assert_eq!(row_indices(&typed, C_AST_KIND_UNION_DECL), vec![13]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ENUM_DECL), vec![21]);
    assert_eq!(
        row_indices(&typed, node_kind::FUNCTION_DECL),
        Vec::<usize>::new()
    );
}

#[test]
fn forward_opaque_tags_are_not_flat_keyword_noise() {
    let fixture = build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("opaque", TOK_IDENTIFIER),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("union", TOK_IDENTIFIER),
        FixtureToken::new("payload", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("enum", TOK_IDENTIFIER),
        FixtureToken::new("state", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let typed = classify(&fixture);
    assert_eq!(row_indices(&typed, C_AST_KIND_STRUCT_DECL), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_UNION_DECL), vec![5]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ENUM_DECL), vec![8]);
}

#[test]
fn typedef_and_function_definition_have_distinct_contract_rows() {
    let fixture = build_fixture(&[
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new("size_t", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("decl", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("defn", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("return", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ]);
    let typed = classify(&fixture);
    assert_eq!(row_indices(&typed, C_AST_KIND_TYPEDEF_DECL), vec![0]);
    assert_eq!(row_indices(&typed, node_kind::FUNCTION_DECL), vec![6]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION),
        vec![13]
    );
}

#[test]
fn static_assert_is_a_declaration_node() {
    let fixture = build_fixture(&[
        FixtureToken::new("_Static_assert", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("\"ok\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ]);
    let typed = classify(&fixture);
    assert_eq!(row_indices(&typed, C_AST_KIND_STATIC_ASSERT_DECL), vec![0]);
}

//! End-to-end C AST tests for char array string-literal initializers.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_vast_nodes, reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_DECL,
    C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_FIELD_DECL, C_AST_KIND_INITIALIZER_LIST,
    C_AST_KIND_MEMBER_ACCESS_EXPR,
};
use vyre_primitives::predicate::node_kind;

const PG_STRIDE_U32: usize = 6;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{
    run_gpu_classifier, run_gpu_pg_lower, run_gpu_vast_builder_from_parts, starts_for_lens,
    word_at, VAST_STRIDE_U32,
};

struct PipelineRows {
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    typed_vast: Vec<u8>,
    pg_nodes: Vec<u8>,
}

fn typed_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn vast_word_at(rows: &[u8], idx: usize, field: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32 + field)
}

fn pg_word_at(rows: &[u8], idx: usize, field: usize) -> u32 {
    word_at(rows, idx * PG_STRIDE_U32 + field)
}

fn run_pipeline(tok_types: &[u32], tok_lens: &[u32]) -> PipelineRows {
    let tok_starts = starts_for_lens(tok_lens);
    let raw_vast = reference_c11_build_vast_nodes(tok_types, &tok_starts, tok_lens);
    let gpu_raw = run_gpu_vast_builder_from_parts(tok_types, &tok_starts, tok_lens);
    assert_eq!(
        gpu_raw, raw_vast,
        "GPU VAST builder must match CPU oracle for string initializer fixture"
    );

    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let gpu_typed = run_gpu_classifier(&raw_vast);
    assert_eq!(
        gpu_typed, typed_vast,
        "GPU VAST classifier must match CPU oracle for string initializer fixture"
    );

    let pg_nodes = reference_ast_to_pg_nodes(&typed_vast);
    let gpu_pg = run_gpu_pg_lower(&typed_vast);
    assert_eq!(
        gpu_pg, pg_nodes,
        "GPU PG lowerer must match CPU oracle for string initializer fixture"
    );

    PipelineRows {
        tok_starts,
        tok_lens: tok_lens.to_vec(),
        typed_vast,
        pg_nodes,
    }
}

fn assert_kind(rows: &PipelineRows, idx: usize, kind: u32) {
    assert_eq!(
        vast_word_at(&rows.typed_vast, idx, 0),
        kind,
        "typed VAST kind at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 0),
        kind,
        "PG kind at row {idx}"
    );
}

fn assert_span(rows: &PipelineRows, idx: usize) {
    let start = rows.tok_starts[idx];
    let end = start + rows.tok_lens[idx];
    assert_eq!(
        vast_word_at(&rows.typed_vast, idx, 5),
        start,
        "typed VAST span_start at row {idx}"
    );
    assert_eq!(
        vast_word_at(&rows.typed_vast, idx, 6),
        rows.tok_lens[idx],
        "typed VAST span_len at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 1),
        start,
        "PG span_start at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 2),
        end,
        "PG span_end at row {idx}"
    );
}

fn assert_links_lowered(rows: &PipelineRows, idx: usize) {
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 3),
        vast_word_at(&rows.typed_vast, idx, 1),
        "PG parent link at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 4),
        vast_word_at(&rows.typed_vast, idx, 2),
        "PG first_child link at row {idx}"
    );
    assert_eq!(
        pg_word_at(&rows.pg_nodes, idx, 5),
        vast_word_at(&rows.typed_vast, idx, 3),
        "PG next_sibling link at row {idx}"
    );
}

fn assert_string_initializer_shape(
    rows: &PipelineRows,
    array_idx: usize,
    assign_idx: usize,
    literal_idx: usize,
) {
    assert_kind(rows, array_idx, C_AST_KIND_ARRAY_DECL);
    assert_kind(rows, assign_idx, C_AST_KIND_ASSIGN_EXPR);
    assert_kind(rows, literal_idx, node_kind::LITERAL);

    assert_eq!(
        vast_word_at(&rows.typed_vast, array_idx, 3),
        assign_idx as u32,
        "array declarator row {array_idx} must be followed by initializer assignment"
    );
    assert_eq!(
        vast_word_at(&rows.typed_vast, assign_idx, 3),
        literal_idx as u32,
        "assignment row {assign_idx} must be followed by string literal initializer"
    );

    for idx in [array_idx, assign_idx, literal_idx] {
        assert_span(rows, idx);
        assert_links_lowered(rows, idx);
    }
}

/// ```c
/// char a[] = "x";
/// ```
fn char_unsized_array_string_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![4, 1, 1, 1, 1, 3, 1];
    (tok_types, tok_lens)
}

/// ```c
/// static const char name[5] = "vyre";
/// ```
fn static_const_char_array_string_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STATIC,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![6, 5, 4, 4, 1, 1, 1, 1, 6, 1];
    (tok_types, tok_lens)
}

/// ```c
/// struct Holder { struct { char code[4]; } nested; };
/// struct Holder h = { .nested = { .code = "abc" } };
/// ```
fn nested_struct_char_array_string_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_LBRACE,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_STRING,
        TOK_RBRACE,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        6, 6, 1, 6, 1, 4, 4, 1, 1, 1, 1, 1, 6, 1, 1, 1, 6, 6, 1, 1, 1, 1, 6, 1, 1, 1, 4, 1, 5, 1,
        1, 1,
    ];
    (tok_types, tok_lens)
}

#[test]
fn char_unsized_array_initializes_from_string_literal() {
    let (tok_types, tok_lens) = char_unsized_array_string_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_string_initializer_shape(&rows, 2, 4, 5);
    assert_eq!(
        typed_indices(&rows.typed_vast, C_AST_KIND_ARRAY_DECL),
        vec![2],
        "unsized char array initializer must produce exactly one array declarator"
    );
    assert_eq!(
        typed_indices(&rows.typed_vast, node_kind::LITERAL),
        vec![5],
        "string initializer must classify as the only literal"
    );
}

#[test]
fn static_const_char_array_preserves_bound_and_string_initializer() {
    let (tok_types, tok_lens) = static_const_char_array_string_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_string_initializer_shape(&rows, 4, 7, 8);
    assert_kind(&rows, 5, node_kind::LITERAL);
    assert_span(&rows, 5);
    assert_links_lowered(&rows, 5);
    assert_eq!(
        vast_word_at(&rows.typed_vast, 4, 2),
        5,
        "array declarator must retain the explicit bound as its first child"
    );
}

#[test]
fn nested_struct_field_char_array_initializes_from_string_literal() {
    let (tok_types, tok_lens) = nested_struct_char_array_string_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_kind(&rows, 7, C_AST_KIND_ARRAY_DECL);
    assert_kind(&rows, 27, C_AST_KIND_ASSIGN_EXPR);
    assert_kind(&rows, 28, node_kind::LITERAL);
    assert_eq!(
        vast_word_at(&rows.typed_vast, 7, 2),
        8,
        "nested char array declarator must retain its explicit bound as first child"
    );
    assert_eq!(
        vast_word_at(&rows.typed_vast, 27, 3),
        28,
        ".code assignment must be followed by the string literal initializer"
    );
    assert_eq!(
        typed_indices(&rows.typed_vast, C_AST_KIND_FIELD_DECL),
        vec![6, 12],
        "nested char array field and nested aggregate field must remain field declarations"
    );
    assert_eq!(
        typed_indices(&rows.typed_vast, C_AST_KIND_INITIALIZER_LIST),
        vec![20, 24],
        "outer and nested designated initializers must both materialize"
    );
    assert_eq!(
        typed_indices(&rows.typed_vast, C_AST_KIND_MEMBER_ACCESS_EXPR),
        vec![21, 25],
        ".nested and .code designators must both materialize"
    );
    assert_eq!(
        typed_indices(&rows.typed_vast, C_AST_KIND_ARRAY_DECL),
        vec![7],
        "nested field char array must produce exactly one array declarator"
    );
    for idx in [6usize, 7, 12, 20, 21, 24, 25, 27, 28] {
        assert_span(&rows, idx);
        assert_links_lowered(&rows, idx);
    }
}

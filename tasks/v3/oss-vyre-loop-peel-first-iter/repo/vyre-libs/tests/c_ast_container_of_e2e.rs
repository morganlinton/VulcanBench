//! End-to-end C parser coverage for container_of-style cast/member expressions.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_CAST_EXPR, C_AST_KIND_MEMBER_ACCESS_EXPR,
    C_AST_KIND_POINTER_DECL, C_EXPR_ASSOC_LEFT, C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_NONE,
    C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;

struct PipelineRows {
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    typed_vast: Vec<u8>,
    expr_shape: Vec<u8>,
    pg_nodes: Vec<u8>,
}

fn starts_for_lens(lens: &[u32]) -> Vec<u32> {
    let mut cursor = 0u32;
    lens.iter()
        .map(|len| {
            let start = cursor;
            cursor = cursor.saturating_add(*len).saturating_add(1);
            start
        })
        .collect()
}

fn word_at(bytes: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    u32::try_from(vast.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

fn row_indices(rows: &[u8], stride_words: usize, kind: u32) -> Vec<usize> {
    rows.chunks_exact(stride_words * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

fn run_reference_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    let num_nodes = node_count_from_vast(typed_vast);
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "pg_nodes");
    let output_len = num_nodes.saturating_mul(PG_STRIDE_U32 as u32).max(1) as usize * 4;
    let values = [
        Value::from(typed_vast.to_vec()),
        Value::from(vec![0; output_len]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap_or_else(|error| panic!("Fix: C AST PG lowerer must execute on CPU: {error}"));
    assert_eq!(outputs.len(), 1, "Fix: PG lowerer must emit one buffer");
    outputs[0].to_bytes()
}

fn run_pipeline(tok_types: &[u32], tok_lens: &[u32]) -> PipelineRows {
    let tok_starts = starts_for_lens(tok_lens);
    let raw_vast = reference_c11_build_vast_nodes(tok_types, &tok_starts, tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expr_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
    let pg_nodes = run_reference_pg_lower(&typed_vast);
    assert_eq!(
        pg_nodes,
        reference_ast_to_pg_nodes(&typed_vast),
        "Fix: executable PG lowerer must match the byte oracle"
    );

    PipelineRows {
        tok_starts,
        tok_lens: tok_lens.to_vec(),
        typed_vast,
        expr_shape,
        pg_nodes,
    }
}

fn assert_kind(rows: &[u8], idx: usize, stride_words: usize, kind: u32) {
    assert_eq!(word_at(rows, idx * stride_words), kind, "kind[{idx}]");
}

fn assert_vast_span(rows: &PipelineRows, idx: usize) {
    assert_eq!(
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 5),
        rows.tok_starts[idx],
        "typed VAST span_start[{idx}]"
    );
    assert_eq!(
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 6),
        rows.tok_lens[idx],
        "typed VAST span_len[{idx}]"
    );
}

fn assert_pg_preserves_row(rows: &PipelineRows, idx: usize, kind: u32) {
    assert_kind(&rows.typed_vast, idx, VAST_STRIDE_U32, kind);
    assert_kind(&rows.pg_nodes, idx, PG_STRIDE_U32, kind);
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 1),
        rows.tok_starts[idx],
        "PG span_start[{idx}]"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 2),
        rows.tok_starts[idx] + rows.tok_lens[idx],
        "PG span_end[{idx}]"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 3),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent[{idx}]"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 4),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child[{idx}]"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 5),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling[{idx}]"
    );
}

fn assert_binary_shape(rows: &PipelineRows, idx: usize, raw_operator: u32, precedence: u32) {
    let row = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(
        word_at(&rows.expr_shape, row),
        C_EXPR_SHAPE_BINARY,
        "shape_kind[{idx}]"
    );
    assert_eq!(
        word_at(&rows.expr_shape, row + 1),
        idx as u32,
        "source_idx[{idx}]"
    );
    assert_eq!(
        word_at(&rows.expr_shape, row + 2),
        raw_operator,
        "raw_operator[{idx}]"
    );
    assert_eq!(
        word_at(&rows.expr_shape, row + 3),
        precedence,
        "precedence[{idx}]"
    );
    assert_eq!(
        word_at(&rows.expr_shape, row + 4),
        C_EXPR_ASSOC_LEFT,
        "associativity[{idx}]"
    );
}

fn cast_to_pointer_arrow_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1, 1, 6, 4, 1, 1, 1, 1, 2, 4, 1];
    (tok_types, tok_lens)
}

fn nested_char_cast_subtraction_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_CHAR_KW,
        TOK_STAR,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_MINUS,
        TOK_LPAREN,
        TOK_CHAR_KW,
        TOK_STAR,
        TOK_RPAREN,
        TOK_AMP,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_RPAREN,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![
        1, 1, 4, 1, 1, 4, 1, 1, 4, 1, 1, 1, 1, 1, 6, 4, 1, 1, 1, 1, 2, 6, 1, 1,
    ];
    (tok_types, tok_lens)
}

#[test]
fn cast_to_pointer_then_arrow_is_cast_pointer_decl_and_member_access() {
    let (tok_types, tok_lens) = cast_to_pointer_arrow_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_CAST_EXPR),
        vec![1],
        "Fix: ((struct node *)p)->member must classify the type-name paren as a cast"
    );
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_POINTER_DECL),
        vec![4],
        "Fix: star inside the cast type-name must be a pointer declarator"
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![8],
        "Fix: arrow after the casted pointer must be member access"
    );
    assert!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::CALL).is_empty(),
        "Fix: cast parentheses must not be typed as CALL nodes"
    );

    for (idx, kind) in [
        (1usize, C_AST_KIND_CAST_EXPR),
        (4, C_AST_KIND_POINTER_DECL),
        (8, C_AST_KIND_MEMBER_ACCESS_EXPR),
    ] {
        assert_vast_span(&rows, idx);
        assert_pg_preserves_row(&rows, idx, kind);
        assert_eq!(
            word_at(&rows.expr_shape, idx * C_EXPR_SHAPE_STRIDE_U32 as usize),
            C_EXPR_SHAPE_NONE,
            "postfix cast/member rows do not receive binary expression-shape rows"
        );
    }

    assert_eq!(tok_types[8], TOK_ARROW);
    assert_eq!(rows.tok_lens[8], 2);
}

#[test]
fn nested_char_pointer_cast_subtraction_preserves_casts_binary_and_arrow() {
    let (tok_types, tok_lens) = nested_char_cast_subtraction_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_CAST_EXPR),
        vec![1, 7, 13],
        "Fix: char* and nested struct-pointer casts must all classify as CAST_EXPR"
    );
    assert_eq!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, C_AST_KIND_POINTER_DECL),
        vec![3, 9, 16],
        "Fix: every star in the cast type-names must be POINTER_DECL"
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![20],
        "Fix: nested zero-pointer member selection must remain MEMBER_ACCESS_EXPR"
    );
    assert_kind(&rows.typed_vast, 6, VAST_STRIDE_U32, node_kind::BINARY);
    assert_binary_shape(&rows, 6, TOK_MINUS, 12);
    assert!(
        row_indices(&rows.typed_vast, VAST_STRIDE_U32, node_kind::CALL).is_empty(),
        "Fix: cast-heavy container_of expressions must not manufacture CALL nodes"
    );

    for (idx, kind) in [
        (1usize, C_AST_KIND_CAST_EXPR),
        (3, C_AST_KIND_POINTER_DECL),
        (6, node_kind::BINARY),
        (7, C_AST_KIND_CAST_EXPR),
        (9, C_AST_KIND_POINTER_DECL),
        (13, C_AST_KIND_CAST_EXPR),
        (16, C_AST_KIND_POINTER_DECL),
        (20, C_AST_KIND_MEMBER_ACCESS_EXPR),
    ] {
        assert_vast_span(&rows, idx);
        assert_pg_preserves_row(&rows, idx, kind);
    }

    assert_eq!(tok_types[6], TOK_MINUS);
    assert_eq!(tok_types[20], TOK_ARROW);
    assert_eq!(rows.tok_lens[20], 2);
}

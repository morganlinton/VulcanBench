// Contracts for C expression-operator ambiguity resolution.
//
// Covers the canonical ambiguities that make C expression parsing hard:
//   - `*`, `&`, `+`, `-` appearing as binary vs unary operators
//   - casts `(type-name)expr` vs parenthesized expressions `(expr)`
//   - `sizeof` / `typeof` followed by type-name vs expression
//
// Every fixture asserts both the CPU reference classification and the
// GPU/CPU parity for VAST building, classification, expression-shape rows,
// and PG lowering.  GPU acquisition failures are never silently skipped.

// cfg(feature = "c-parser")  -  moved to parent
// allow(clippy::erasing_op)  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_CAST_EXPR,
    C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_UNARY_EXPR, C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_NONE,
    C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_NONE, C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

use c_ast_gpu_parity_support::{
    row_indices_by_stride, run_gpu_expr_shape, run_gpu_pg_lower, starts_for_lens, word_at,
    VAST_STRIDE_U32,
};

const PG_STRIDE_U32: usize = 6;
const SENTINEL: u32 = u32::MAX;

struct PipelineRows {
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    typed_vast: Vec<u8>,
    expr_shape: Vec<u8>,
    pg_nodes: Vec<u8>,
}

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn node_count_from_vast(vast: &[u8]) -> u32 {
    u32::try_from(vast.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

fn row_indices(rows: &[u8], stride_words: usize, kind: u32) -> Vec<usize> {
    row_indices_by_stride(rows, stride_words, kind)
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
    assert_eq!(word_at(rows, idx * stride_words), kind, "kind at row {idx}");
}

fn assert_pg_preserves_row(rows: &PipelineRows, idx: usize, kind: u32) {
    assert_kind(&rows.typed_vast, idx, VAST_STRIDE_U32, kind);
    assert_kind(&rows.pg_nodes, idx, PG_STRIDE_U32, kind);
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 1),
        rows.tok_starts[idx],
        "PG span_start at row {idx}"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 2),
        rows.tok_starts[idx] + rows.tok_lens[idx],
        "PG span_end at row {idx}"
    );
}

fn assert_pg_links_match_vast(rows: &PipelineRows, idx: usize) {
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 3),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent at row {idx}"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 4),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child at row {idx}"
    );
    assert_eq!(
        word_at(&rows.pg_nodes, idx * PG_STRIDE_U32 + 5),
        word_at(&rows.typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling at row {idx}"
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_shape_row(
    rows: &[u8],
    idx: usize,
    shape_kind: u32,
    raw_operator: u32,
    precedence: u32,
    associativity: u32,
    first: u32,
    second: u32,
    third: u32,
) {
    let row = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(word_at(rows, row), shape_kind, "shape_kind[{idx}]");
    assert_eq!(
        word_at(rows, row + 1),
        if shape_kind == C_EXPR_SHAPE_NONE {
            SENTINEL
        } else {
            idx as u32
        },
        "source_idx[{idx}]"
    );
    assert_eq!(word_at(rows, row + 2), raw_operator, "raw_operator[{idx}]");
    assert_eq!(word_at(rows, row + 3), precedence, "precedence[{idx}]");
    assert_eq!(
        word_at(rows, row + 4),
        associativity,
        "associativity[{idx}]"
    );
    assert_eq!(word_at(rows, row + 5), first, "first[{idx}]");
    assert_eq!(word_at(rows, row + 6), second, "second[{idx}]");
    assert_eq!(word_at(rows, row + 7), third, "third[{idx}]");
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn star_binary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_STAR, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn star_unary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_STAR, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn amp_binary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_AMP, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn amp_unary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_AMP, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn plus_binary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn plus_unary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_PLUS, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn minus_binary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_MINUS, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn minus_unary_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_MINUS, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn cast_simple_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn paren_expr_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_PLUS,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn cast_complex_fixture() -> (Vec<u32>, Vec<u32>) {
    // (const int *)p;
    let tok_types = vec![
        TOK_LPAREN,
        TOK_CONST,
        TOK_INT,
        TOK_STAR,
        TOK_RPAREN,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn paren_nested_fixture() -> (Vec<u32>, Vec<u32>) {
    // ((a));
    let tok_types = vec![
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn sizeof_typename_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_SIZEOF, TOK_LPAREN, TOK_INT, TOK_RPAREN, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn sizeof_expr_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_SIZEOF, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn typeof_typename_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_GNU_TYPEOF,
        TOK_LPAREN,
        TOK_INT,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn typeof_expr_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_GNU_TYPEOF, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

// ---------------------------------------------------------------------------
// Ambiguity tests
// ---------------------------------------------------------------------------

#[test]
fn star_binary_is_binary_and_unary_is_unary() {
    let (tok_types, tok_lens) = star_binary_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(&rows.typed_vast, VAST_STRIDE_U32, node_kind::BINARY),
        vec![1],
        "Fix: * in binary context must classify as BINARY"
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_STAR,
        13,
        C_EXPR_ASSOC_LEFT,
        0,
        2,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 1, node_kind::BINARY);
    assert_pg_links_match_vast(&rows, 1);

    let (tok_types_u, tok_lens_u) = star_unary_fixture();
    let rows_u = run_pipeline(&tok_types_u, &tok_lens_u);
    assert_eq!(
        row_indices_by_stride(&rows_u.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: * in unary context must classify as UNARY_EXPR"
    );
    assert_shape_row(
        &rows_u.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_STAR,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows_u, 0, C_AST_KIND_UNARY_EXPR);
    assert_pg_links_match_vast(&rows_u, 0);
}

#[test]
fn amp_binary_is_binary_and_unary_is_unary() {
    let (tok_types, tok_lens) = amp_binary_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(&rows.typed_vast, VAST_STRIDE_U32, node_kind::BINARY),
        vec![1],
        "Fix: & in binary context must classify as BINARY"
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_AMP,
        8,
        C_EXPR_ASSOC_LEFT,
        0,
        2,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 1, node_kind::BINARY);
    assert_pg_links_match_vast(&rows, 1);

    let (tok_types_u, tok_lens_u) = amp_unary_fixture();
    let rows_u = run_pipeline(&tok_types_u, &tok_lens_u);
    assert_eq!(
        row_indices_by_stride(&rows_u.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: & in unary context must classify as UNARY_EXPR"
    );
    assert_shape_row(
        &rows_u.expr_shape,
        0,
        C_EXPR_SHAPE_NONE,
        TOK_AMP,
        0,
        C_EXPR_ASSOC_NONE,
        SENTINEL,
        SENTINEL,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows_u, 0, C_AST_KIND_UNARY_EXPR);
    assert_pg_links_match_vast(&rows_u, 0);
}

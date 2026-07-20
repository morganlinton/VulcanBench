// Contracts for C builtin and generic-selection expression classification.
//
// Covers:
//   - `__builtin_constant_p`
//   - `__builtin_choose_expr`
//   - `__builtin_types_compatible_p`
//   - C11 `_Generic`
//   - Nested builtin/generic combinations
//
// Every test asserts that these expressions receive distinct VAST kinds and
// do NOT collapse into generic `CALL` or `BINARY`.  GPU/CPU parity and PG
// lowering preservation are asserted for every fixture.

// cfg(feature = "c-parser")  -  moved to parent
// allow(clippy::erasing_op)  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds,
    C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
    C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR, C_AST_KIND_GENERIC_SELECTION_EXPR,
    C_EXPR_ASSOC_NONE, C_EXPR_SHAPE_NONE, C_EXPR_SHAPE_STRIDE_U32,
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

fn assert_shape_none(rows: &[u8], idx: usize, raw_operator: u32) {
    let row = idx * C_EXPR_SHAPE_STRIDE_U32 as usize;
    assert_eq!(word_at(rows, row), C_EXPR_SHAPE_NONE, "shape_kind[{idx}]");
    assert_eq!(word_at(rows, row + 1), SENTINEL, "source_idx[{idx}]");
    assert_eq!(word_at(rows, row + 2), raw_operator, "raw_operator[{idx}]");
    assert_eq!(word_at(rows, row + 3), 0, "precedence[{idx}]");
    assert_eq!(
        word_at(rows, row + 4),
        C_EXPR_ASSOC_NONE,
        "associativity[{idx}]"
    );
    assert_eq!(word_at(rows, row + 5), SENTINEL, "first[{idx}]");
    assert_eq!(word_at(rows, row + 6), SENTINEL, "second[{idx}]");
    assert_eq!(word_at(rows, row + 7), SENTINEL, "third[{idx}]");
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn builtin_constant_p_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_BUILTIN_CONSTANT_P,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn builtin_choose_expr_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_BUILTIN_CHOOSE_EXPR,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn builtin_types_compatible_p_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_BUILTIN_TYPES_COMPATIBLE_P,
        TOK_LPAREN,
        TOK_INT,
        TOK_COMMA,
        TOK_LONG,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn generic_selection_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_GENERIC,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_INT,
        TOK_COLON,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_DEFAULT,
        TOK_COLON,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn nested_builtin_fixture() -> (Vec<u32>, Vec<u32>) {
    // __builtin_choose_expr(1, __builtin_constant_p(2), 0);
    let tok_types = vec![
        TOK_BUILTIN_CHOOSE_EXPR,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_BUILTIN_CONSTANT_P,
        TOK_LPAREN,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_COMMA,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn builtin_constant_p_classifies_as_distinct_expr() {
    let (tok_types, tok_lens) = builtin_constant_p_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_BUILTIN_CONSTANT_P_EXPR
        ),
        vec![0],
        "Fix: __builtin_constant_p must be a distinct expression kind"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::CALL,
        "Fix: __builtin_constant_p must not collapse into CALL"
    );
    assert_shape_none(&rows.expr_shape, 0, TOK_BUILTIN_CONSTANT_P);
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR);
    assert_pg_links_match_vast(&rows, 0);
}

#[test]
fn builtin_choose_expr_classifies_as_distinct_expr() {
    let (tok_types, tok_lens) = builtin_choose_expr_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_BUILTIN_CHOOSE_EXPR
        ),
        vec![0],
        "Fix: __builtin_choose_expr must be a distinct expression kind"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::CALL,
        "Fix: __builtin_choose_expr must not collapse into CALL"
    );
    assert_shape_none(&rows.expr_shape, 0, TOK_BUILTIN_CHOOSE_EXPR);
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
    assert_pg_links_match_vast(&rows, 0);
}

#[test]
fn builtin_types_compatible_p_classifies_as_distinct_expr() {
    let (tok_types, tok_lens) = builtin_types_compatible_p_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR
        ),
        vec![0],
        "Fix: __builtin_types_compatible_p must be a distinct expression kind"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::CALL,
        "Fix: __builtin_types_compatible_p must not collapse into CALL"
    );
    assert_shape_none(&rows.expr_shape, 0, TOK_BUILTIN_TYPES_COMPATIBLE_P);
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR);
    assert_pg_links_match_vast(&rows, 0);
}

#[test]
fn generic_selection_classifies_as_distinct_expr() {
    let (tok_types, tok_lens) = generic_selection_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_GENERIC_SELECTION_EXPR
        ),
        vec![0],
        "Fix: _Generic must be a distinct selection-expression kind"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::CALL,
        "Fix: _Generic must not collapse into CALL"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "Fix: _Generic must not collapse into BINARY"
    );
    assert_shape_none(&rows.expr_shape, 0, TOK_GENERIC);
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_GENERIC_SELECTION_EXPR);
    assert_pg_links_match_vast(&rows, 0);
}

#[test]
fn nested_builtin_and_generic_expressions_classify_correctly() {
    let (tok_types, tok_lens) = nested_builtin_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_BUILTIN_CHOOSE_EXPR
        ),
        vec![0],
        "Fix: outer __builtin_choose_expr must classify"
    );
    assert_eq!(
        row_indices_by_stride(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_BUILTIN_CONSTANT_P_EXPR
        ),
        vec![4],
        "Fix: nested __builtin_constant_p must classify"
    );

    for idx in [0usize, 4] {
        assert_ne!(
            word_at(&rows.typed_vast, idx * VAST_STRIDE_U32),
            node_kind::CALL,
            "Fix: builtin row {idx} must not collapse into CALL"
        );
    }

    assert_pg_preserves_row(&rows, 0, C_AST_KIND_BUILTIN_CHOOSE_EXPR);
    assert_pg_preserves_row(&rows, 4, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR);
    assert_pg_links_match_vast(&rows, 0);
    assert_pg_links_match_vast(&rows, 4);
}

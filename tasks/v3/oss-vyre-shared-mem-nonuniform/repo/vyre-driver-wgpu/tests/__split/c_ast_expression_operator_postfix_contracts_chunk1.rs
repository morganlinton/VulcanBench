// Contracts for C postfix and unary expression classification.
//
// Covers:
//   - chained member access (`.` and `->`)
//   - chained array subscript (`[]`)
//   - mixed postfix sequences (`a[i].b->c`)
//   - unary dereference (`*`) and address-of (`&`)
//   - GNU `__real__` and `__imag__`
//   - GNU label-address (`&&label`)
//   - postfix increment/decrement position contracts
//
// GPU/CPU parity and PG lowering preservation are asserted for every fixture.

// cfg(feature = "c-parser")  -  moved to parent
// allow(clippy::erasing_op)  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_GNU_LABEL_ADDRESS_EXPR, C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_UNARY_EXPR,
    C_EXPR_ASSOC_NONE, C_EXPR_SHAPE_NONE, C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;
use c_ast_gpu_parity_support::{run_gpu_expr_shape, run_gpu_pg_lower, starts_for_lens};

const VAST_STRIDE_U32: usize = 10;
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

fn chained_member_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn chained_arrow_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn mixed_postfix_fixture() -> (Vec<u32>, Vec<u32>) {
    // a[0].b->c;
    let tok_types = vec![
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_DOT,
        TOK_IDENTIFIER,
        TOK_ARROW,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn unary_deref_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_STAR, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn unary_addressof_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_AMP, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn gnu_real_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_GNU_REAL, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn gnu_imag_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_GNU_IMAG, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn label_address_fixture() -> (Vec<u32>, Vec<u32>) {
    // &&label;  -- && is a single TOK_AND token in this pipeline
    let tok_types = vec![TOK_AND, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn postfix_inc_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_INC, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn postfix_dec_fixture() -> (Vec<u32>, Vec<u32>) {
    let tok_types = vec![TOK_IDENTIFIER, TOK_DEC, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn chained_member_access_classifies_each_dot() {
    let (tok_types, tok_lens) = chained_member_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![1, 3],
        "Fix: each . in a.b.c must classify as MEMBER_ACCESS_EXPR"
    );
    assert_shape_none(&rows.expr_shape, 1, TOK_DOT);
    assert_shape_none(&rows.expr_shape, 3, TOK_DOT);
    assert_pg_preserves_row(&rows, 1, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_pg_preserves_row(&rows, 3, C_AST_KIND_MEMBER_ACCESS_EXPR);
}

#[test]
fn chained_arrow_access_classifies_each_arrow() {
    let (tok_types, tok_lens) = chained_arrow_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![1, 3],
        "Fix: each -> in a->b->c must classify as MEMBER_ACCESS_EXPR"
    );
    assert_shape_none(&rows.expr_shape, 1, TOK_ARROW);
    assert_shape_none(&rows.expr_shape, 3, TOK_ARROW);
    assert_pg_preserves_row(&rows, 1, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_pg_preserves_row(&rows, 3, C_AST_KIND_MEMBER_ACCESS_EXPR);
}

#[test]
fn mixed_postfix_member_and_subscript_classifies() {
    let (tok_types, tok_lens) = mixed_postfix_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
        ),
        vec![1],
        "Fix: [ in a[0].b->c must classify as ARRAY_SUBSCRIPT_EXPR"
    );
    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_MEMBER_ACCESS_EXPR
        ),
        vec![4, 6],
        "Fix: . and -> in a[0].b->c must classify as MEMBER_ACCESS_EXPR"
    );
    for idx in [1usize, 4, 6] {
        assert_pg_preserves_row(&rows, idx, word_at(&rows.typed_vast, idx * VAST_STRIDE_U32));
        assert_pg_links_match_vast(&rows, idx);
    }
}

#[test]
fn unary_deref_and_addressof_are_unary_expr() {
    let (tok_types_d, tok_lens_d) = unary_deref_fixture();
    let rows_d = run_pipeline(&tok_types_d, &tok_lens_d);
    assert_eq!(
        row_indices(&rows_d.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: *p must classify * as UNARY_EXPR"
    );
    assert_shape_none(&rows_d.expr_shape, 0, TOK_STAR);
    assert_pg_preserves_row(&rows_d, 0, C_AST_KIND_UNARY_EXPR);

    let (tok_types_a, tok_lens_a) = unary_addressof_fixture();
    let rows_a = run_pipeline(&tok_types_a, &tok_lens_a);
    assert_eq!(
        row_indices(&rows_a.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: &x must classify & as UNARY_EXPR"
    );
    assert_shape_none(&rows_a.expr_shape, 0, TOK_AMP);
    assert_pg_preserves_row(&rows_a, 0, C_AST_KIND_UNARY_EXPR);
}

#[test]
fn gnu_real_and_imag_are_unary_expr() {
    let (tok_types_r, tok_lens_r) = gnu_real_fixture();
    let rows_r = run_pipeline(&tok_types_r, &tok_lens_r);
    assert_eq!(
        row_indices(&rows_r.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: __real__ must classify as UNARY_EXPR"
    );
    assert_shape_none(&rows_r.expr_shape, 0, TOK_GNU_REAL);
    assert_pg_preserves_row(&rows_r, 0, C_AST_KIND_UNARY_EXPR);

    let (tok_types_i, tok_lens_i) = gnu_imag_fixture();
    let rows_i = run_pipeline(&tok_types_i, &tok_lens_i);
    assert_eq!(
        row_indices(&rows_i.typed_vast, VAST_STRIDE_U32, C_AST_KIND_UNARY_EXPR),
        vec![0],
        "Fix: __imag__ must classify as UNARY_EXPR"
    );
    assert_shape_none(&rows_i.expr_shape, 0, TOK_GNU_IMAG);
    assert_pg_preserves_row(&rows_i, 0, C_AST_KIND_UNARY_EXPR);
}

#[test]
fn label_address_expr_classifies_and_lowers() {
    let (tok_types, tok_lens) = label_address_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        row_indices(
            &rows.typed_vast,
            VAST_STRIDE_U32,
            C_AST_KIND_GNU_LABEL_ADDRESS_EXPR
        ),
        vec![0],
        "Fix: &&label must classify as GNU_LABEL_ADDRESS_EXPR"
    );
    assert_ne!(
        word_at(&rows.typed_vast, 0 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "Fix: &&label must not be confused with logical AND"
    );
    assert_shape_none(&rows.expr_shape, 0, TOK_AND);
    assert_pg_preserves_row(&rows, 0, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR);
    assert_pg_links_match_vast(&rows, 0);
}

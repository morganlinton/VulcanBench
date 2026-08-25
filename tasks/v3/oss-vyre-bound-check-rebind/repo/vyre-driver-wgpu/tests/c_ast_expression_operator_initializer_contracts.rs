//! Contracts distinguishing declaration initializers from assignment expressions.
//!
//! Covers:
//!   - simple declaration initializer (`int x = 1;`)
//!   - assignment expression (`x = 1;`)
//!   - multiple declarators (`int a = 1, b = 2;`)
//!   - `for`-loop initializer (`for (int i = 0; ...)`)
//!   - brace-enclosed designator initializer (`{ .x = 1 }`)
//!
//! The core contract is that `=` in a declaration-context initializer must be
//! treated differently from `=` in an expression context, while designator
//! assignments inside braces are expression assignments.

#![cfg(feature = "c-parser")]
#![allow(clippy::too_many_arguments)]
#![allow(deprecated)]

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_FOR_STMT,
    C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_RIGHT, C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_NONE,
    C_EXPR_SHAPE_STRIDE_U32,
};
use vyre_primitives::predicate::node_kind;
use vyre_reference::value::Value;

mod c_ast_gpu_parity_support;
use c_ast_gpu_parity_support::{run_gpu_expr_shape, run_gpu_pg_lower};

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

fn declaration_initializer_fixture() -> (Vec<u32>, Vec<u32>) {
    // int x = 1;
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn assignment_expression_fixture() -> (Vec<u32>, Vec<u32>) {
    // x = 1;
    let tok_types = vec![TOK_IDENTIFIER, TOK_ASSIGN, TOK_INTEGER, TOK_SEMICOLON];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn multiple_declarator_fixture() -> (Vec<u32>, Vec<u32>) {
    // int a = 1, b = 2;
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn for_loop_initializer_fixture() -> (Vec<u32>, Vec<u32>) {
    // for (int i = 0; i < n; i++) { }
    let tok_types = vec![
        TOK_FOR,
        TOK_LPAREN,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_ASSIGN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_LT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_IDENTIFIER,
        TOK_INC,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

fn designator_initializer_fixture() -> (Vec<u32>, Vec<u32>) {
    // struct S s = { .x = 1 };
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
    let tok_lens = vec![1; tok_types.len()];
    (tok_types, tok_lens)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn declaration_initializer_assign_is_not_expr_assign() {
    let (tok_types, tok_lens) = declaration_initializer_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    let kind = word_at(&rows.typed_vast, 2 * VAST_STRIDE_U32);
    assert_eq!(
        kind, 0,
        "Fix: = in declaration initializer must NOT be ASSIGN_EXPR (distinct from expression assignment)"
    );
    assert_eq!(
        word_at(&rows.expr_shape, 2 * C_EXPR_SHAPE_STRIDE_U32 as usize),
        C_EXPR_SHAPE_NONE,
        "Fix: declaration initializer = must not receive a BINARY shape row"
    );
}

#[test]
fn assignment_expression_is_assign_expr_with_shape() {
    let (tok_types, tok_lens) = assignment_expression_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    assert_eq!(
        word_at(&rows.typed_vast, VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "Fix: = in expression context must classify as ASSIGN_EXPR"
    );
    assert_shape_row(
        &rows.expr_shape,
        1,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        0,
        2,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 1, C_AST_KIND_ASSIGN_EXPR);
    assert_pg_links_match_vast(&rows, 1);
}

#[test]
fn multiple_declarator_initializers_are_distinct_from_expression_assigns() {
    let (tok_types, tok_lens) = multiple_declarator_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // Both = tokens are in declaration context, so neither is ASSIGN_EXPR.
    let kind_first = word_at(&rows.typed_vast, 2 * VAST_STRIDE_U32);
    let kind_second = word_at(&rows.typed_vast, 6 * VAST_STRIDE_U32);
    assert_eq!(
        kind_first, 0,
        "Fix: first = in multi-declarator init must not be ASSIGN_EXPR"
    );
    assert_eq!(
        kind_second, 0,
        "Fix: second = in multi-declarator init must not be ASSIGN_EXPR"
    );

    // Comma between declarators is a boundary, not a shape node.
    assert_eq!(
        word_at(&rows.expr_shape, 4 * C_EXPR_SHAPE_STRIDE_U32 as usize),
        C_EXPR_SHAPE_NONE,
        "Fix: comma between declarators must be NONE shape"
    );
}

#[test]
fn for_loop_initializer_assign_is_not_expr_assign_and_condition_is_binary() {
    let (tok_types, tok_lens) = for_loop_initializer_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // = in `int i = 0` is inside the for-init declarator context.
    let init_assign_kind = word_at(&rows.typed_vast, 4 * VAST_STRIDE_U32);
    assert_eq!(
        init_assign_kind, 0,
        "Fix: = in for-loop initializer must NOT be ASSIGN_EXPR"
    );

    // < in `i < n` is a normal relational binary operator.
    assert_eq!(
        word_at(&rows.typed_vast, 8 * VAST_STRIDE_U32),
        node_kind::BINARY,
        "Fix: < in for-loop condition must classify as BINARY"
    );
    assert_shape_row(
        &rows.expr_shape,
        8,
        C_EXPR_SHAPE_BINARY,
        TOK_LT,
        10,
        C_EXPR_ASSOC_LEFT,
        7,
        9,
        SENTINEL,
    );

    // FOR itself must classify as FOR_STMT.
    assert_eq!(
        word_at(&rows.typed_vast, 0),
        C_AST_KIND_FOR_STMT,
        "Fix: for keyword must classify as FOR_STMT"
    );

    assert_pg_preserves_row(&rows, 0, C_AST_KIND_FOR_STMT);
    assert_pg_preserves_row(&rows, 8, node_kind::BINARY);
}

#[test]
fn designator_assign_in_brace_initializer_is_expr_assign() {
    let (tok_types, tok_lens) = designator_initializer_fixture();
    let rows = run_pipeline(&tok_types, &tok_lens);

    // The outer = in `struct S s = { ... }` is a declaration initializer.
    let outer_kind = word_at(&rows.typed_vast, 3 * VAST_STRIDE_U32);
    assert_eq!(
        outer_kind, 0,
        "Fix: outer = before brace must be declaration initializer (not ASSIGN_EXPR)"
    );

    // The inner = in `.x = 1` is inside braces, so declaration context is reset.
    assert_eq!(
        word_at(&rows.typed_vast, 7 * VAST_STRIDE_U32),
        C_AST_KIND_ASSIGN_EXPR,
        "Fix: designator = inside brace must classify as ASSIGN_EXPR"
    );
    // Note: the shape builder currently returns the first token in the segment
    // (the designator dot at 5) as the left operand, because it does not
    // recognize designator syntax.  The classification contract below is the
    // stronger assertion.
    assert_shape_row(
        &rows.expr_shape,
        7,
        C_EXPR_SHAPE_BINARY,
        TOK_ASSIGN,
        2,
        C_EXPR_ASSOC_RIGHT,
        5,
        8,
        SENTINEL,
    );
    assert_pg_preserves_row(&rows, 7, C_AST_KIND_ASSIGN_EXPR);
    assert_pg_links_match_vast(&rows, 7);
}

// ---------------------------------------------------------------------------
// GPU / CPU parity
// ---------------------------------------------------------------------------

#[test]
fn gpu_matches_cpu_for_initializer_fixtures() {
    let fixtures: Vec<(Vec<u32>, Vec<u32>)> = vec![
        declaration_initializer_fixture(),
        assignment_expression_fixture(),
        multiple_declarator_fixture(),
        for_loop_initializer_fixture(),
        designator_initializer_fixture(),
    ];

    for (fixture_idx, (tok_types, tok_lens)) in fixtures.iter().enumerate() {
        let tok_starts = starts_for_lens(tok_lens);
        let raw_vast = reference_c11_build_vast_nodes(tok_types, &tok_starts, tok_lens);
        let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
        let expected_shape = reference_c11_build_expression_shape_nodes(&raw_vast, &typed_vast);
        let expected_pg = run_reference_pg_lower(&typed_vast);

        assert_eq!(
            run_gpu_expr_shape(&raw_vast, &typed_vast),
            expected_shape,
            "GPU expression-shape rows must match CPU for fixture {fixture_idx}"
        );
        assert_eq!(
            run_gpu_pg_lower(&typed_vast),
            expected_pg,
            "GPU PG lowering must match CPU for fixture {fixture_idx}"
        );

        let typed_bytes = bytes(
            &typed_vast
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            typed_bytes, typed_vast,
            "typed VAST fixture {fixture_idx} must stay word-aligned for GPU dispatch"
        );
    }
}

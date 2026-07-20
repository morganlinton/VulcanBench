// Adversarial contract tests for malformed streams and parser stage boundaries.
//
// Covers: misaligned VAST bytes, partial rows, max-size limits, empty inputs,
// single-token/minimal inputs, and the invariant that no stage silently
// produces all-zero or default output for non-empty input.

use std::panic::{catch_unwind, AssertUnwindSafe};

use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{
    c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes, try_reference_ast_to_pg_nodes,
    PgReferenceDecodeError,
};
use vyre_libs::parsing::c::parse::vast::{
    c11_build_expression_shape_nodes, reference_c11_annotate_typedef_names,
    reference_c11_build_expression_shape_nodes, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds, try_reference_c11_build_expression_shape_nodes,
    CReferenceDecodeError, C_EXPR_SHAPE_NONE,
};
use vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN;
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------


const VAST_STRIDE_U32: usize = 10;
const PG_STRIDE_U32: usize = 6;
const SENTINEL: u32 = u32::MAX;

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
        .unwrap_or_else(|e| panic!("PG lowerer must execute on CPU: {e}"));
    assert_eq!(outputs.len(), 1);
    outputs[0].to_bytes()
}

fn run_reference_expr_shape(raw_vast: &[u8], typed_vast: &[u8]) -> Vec<u8> {
    let num_nodes = node_count_from_vast(raw_vast);
    let program = c11_build_expression_shape_nodes(
        "raw_vast_nodes",
        "typed_vast_nodes",
        Expr::u32(num_nodes),
        "expr_shape_nodes",
    );
    let output_len = num_nodes.saturating_mul(8).max(1) as usize * 4;
    let values = [
        Value::from(raw_vast.to_vec()),
        Value::from(typed_vast.to_vec()),
        Value::from(vec![0; output_len]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap_or_else(|e| panic!("expr-shape must execute on CPU: {e}"));
    assert_eq!(outputs.len(), 1);
    outputs[0].to_bytes()
}

// ---------------------------------------------------------------------------
// 1. PG lowering malformed-input contracts
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_try_reference_rejects_misaligned_bytes() {
    let result = try_reference_ast_to_pg_nodes(&[0u8; 14]);
    assert_eq!(
        result,
        Err(PgReferenceDecodeError::MisalignedBytes { len: 14 }),
        "14 bytes (not divisible by 40) must be rejected"
    );
}

#[test]
fn pg_lower_try_reference_rejects_partial_vast_row() {
    // 44 bytes = 11 u32s = 1 full row + 1 extra u32
    let result = try_reference_ast_to_pg_nodes(&[0u8; 44]);
    assert_eq!(
        result,
        Err(PgReferenceDecodeError::PartialVastRow {
            words: 11,
            stride: 10,
        }),
        "44 bytes (1 row + 1 word) must be rejected as partial row"
    );
}

#[test]
fn pg_lower_try_reference_accepts_exact_multiple_of_stride() {
    let mut vast = vec![0u8; VAST_STRIDE_U32 * 4];
    // Write a kind word so it's not all zeros.
    vast[0..4].copy_from_slice(&1u32.to_le_bytes());
    let result = try_reference_ast_to_pg_nodes(&vast);
    assert!(result.is_ok(), "exactly one VAST row must be accepted");
    let pg = result.unwrap();
    assert_eq!(pg.len(), PG_STRIDE_U32 * 4, "one PG row must be emitted");
}

#[test]
fn pg_lower_reference_oracle_panics_on_misaligned_input() {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = reference_ast_to_pg_nodes(&[0u8; 14]);
    }))
    .expect_err("reference_ast_to_pg_nodes must panic on misaligned bytes");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload");
    assert!(
        message.contains("aligned") || message.contains("stride"),
        "unexpected panic payload: {message}"
    );
}

#[test]
fn vast_reference_oracles_panic_on_malformed_public_input() {
    for (name, result) in [
        (
            "classify",
            catch_unwind(AssertUnwindSafe(|| {
                let _ = reference_c11_classify_vast_node_kinds(&[0u8; 14]);
            })),
        ),
        (
            "expr_shape",
            catch_unwind(AssertUnwindSafe(|| {
                let _ = reference_c11_build_expression_shape_nodes(&[0u8; 14], &[]);
            })),
        ),
        (
            "typedef",
            catch_unwind(AssertUnwindSafe(|| {
                let _ = reference_c11_annotate_typedef_names(&[0u8; 14], b"typedef int T;");
            })),
        ),
    ] {
        let panic = result.expect_err(
            "{name} reference oracle must panic on malformed VAST bytes instead of emitting an empty vector",
        );
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        assert!(
            message.contains("aligned") || message.contains("stride"),
            "unexpected {name} panic payload: {message}"
        );
    }
}

#[test]
fn expr_shape_reference_rejects_mismatched_raw_and_typed_rows() {
    let raw = u32_bytes(&[TOK_IDENTIFIER, SENTINEL, SENTINEL, SENTINEL, SENTINEL, 0, 1, 0, 0, 0]);
    let result = try_reference_c11_build_expression_shape_nodes(&raw, &[]);
    assert_eq!(
        result,
        Err(CReferenceDecodeError::MismatchedVastRows {
            raw_rows: 1,
            typed_rows: 0,
        }),
        "expression-shape oracle must reject mismatched raw/typed streams instead of defaulting missing typed rows"
    );

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = reference_c11_build_expression_shape_nodes(&raw, &[]);
    }));
    assert!(
        panic_result.is_err(),
        "public expression-shape oracle must panic on mismatched raw/typed streams"
    );
}

#[test]
fn pg_lower_zero_nodes_fails_validation_on_cpu_reference() {
    // The CPU reference PG lowerer requires at least one VAST row (40 bytes).
    // Zero-node input must fail validation, not silently emit all zeros.
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(0), "pg_nodes");
    let values = [Value::from(vec![]), Value::from(vec![0u8; 4])];
    let err = vyre_reference::reference_eval(&program, &values).expect_err(
        "zero-node PG lowerer must fail validation on CPU reference, not silently succeed",
    );
    assert!(
        err.to_string().contains("vast_nodes") || err.to_string().contains("validate"),
        "unexpected error: {err}"
    );
}

#[test]
fn pg_lower_single_node_preserves_kind_and_spans() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = 42; // kind
    vast[1] = SENTINEL; // parent
    vast[5] = 10; // span_start
    vast[6] = 5; // span_len
    let vast_bytes = u32_bytes(&vast);
    let pg = run_reference_pg_lower(&vast_bytes);
    assert_eq!(pg.len(), PG_STRIDE_U32 * 4);
    assert_eq!(word_at(&pg, 0), 42, "kind preserved");
    assert_eq!(word_at(&pg, 1), 10, "span_start preserved");
    assert_eq!(word_at(&pg, 2), 15, "span_end = start + len");
    assert_eq!(word_at(&pg, 3), SENTINEL, "parent preserved");
}

// ---------------------------------------------------------------------------
// 2. VAST builder malformed / boundary contracts
// ---------------------------------------------------------------------------

#[test]
fn vast_builder_zero_tokens_produces_empty() {
    let raw = reference_c11_build_vast_nodes(&[], &[], &[]);
    assert!(raw.is_empty(), "zero tokens must produce empty VAST");
}

#[test]
fn vast_builder_single_token_no_delimiters() {
    let tok_types = [TOK_IDENTIFIER];
    let tok_starts = [0u32];
    let tok_lens = [3u32];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), VAST_STRIDE_U32 * 4, "single token = one row");
    assert_eq!(word_at(&raw, 0), TOK_IDENTIFIER);
    assert_eq!(word_at(&raw, 1), SENTINEL, "parent");
    assert_eq!(word_at(&raw, 2), SENTINEL, "first_child");
    assert_eq!(word_at(&raw, 3), SENTINEL, "next_sibling");
}

#[test]
fn vast_builder_unmatched_lbrace_produces_rows_without_crash() {
    // The builder is structural, not validating; unmatched delimiters still emit rows.
    let tok_types = [TOK_LBRACE, TOK_IDENTIFIER, TOK_SEMICOLON];
    let tok_starts = [0u32, 2, 4];
    let tok_lens = [1u32; 3];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        3 * VAST_STRIDE_U32 * 4,
        "unmatched lbrace must still produce 3 rows"
    );
}

#[test]
fn vast_builder_unmatched_rbrace_produces_rows_without_crash() {
    let tok_types = [TOK_IDENTIFIER, TOK_RBRACE];
    let tok_starts = [0u32, 2];
    let tok_lens = [1u32; 2];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        2 * VAST_STRIDE_U32 * 4,
        "unmatched rbrace must still produce 2 rows"
    );
}

#[test]
fn vast_builder_unmatched_lparen_produces_rows_without_crash() {
    let tok_types = [TOK_LPAREN, TOK_IDENTIFIER];
    let tok_starts = [0u32, 2];
    let tok_lens = [1u32; 2];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        2 * VAST_STRIDE_U32 * 4,
        "unmatched lparen must still produce 2 rows"
    );
}

// ---------------------------------------------------------------------------
// 3. Classifier boundary contracts
// ---------------------------------------------------------------------------

#[test]
fn classifier_empty_vast_is_empty() {
    let typed = reference_c11_classify_vast_node_kinds(&[]);
    assert!(typed.is_empty(), "empty vast must produce empty typed vast");
}

#[test]
fn classifier_preserves_unknown_kind_as_zero() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = 0xDEAD_BEEF; // unknown raw kind
    vast[5] = 7; // span_start
    vast[6] = 3; // span_len
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    assert_eq!(typed.len(), VAST_STRIDE_U32 * 4);
    assert_eq!(
        word_at(&typed, 0),
        0,
        "unknown raw kind must classify as 0, not silently default to something else"
    );
    assert_eq!(
        word_at(&typed, 5),
        7,
        "span_start must survive classification"
    );
    assert_eq!(
        word_at(&typed, 6),
        3,
        "span_len must survive classification"
    );
}

#[test]
fn classifier_does_not_silently_zero_all_fields() {
    let mut vast = vec![0u32; VAST_STRIDE_U32 * 2];
    vast[0] = TOK_IF;
    vast[10] = TOK_IDENTIFIER;
    vast[10 + 5] = 12;
    vast[10 + 6] = 4;
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    // Second row is an identifier; in typed VAST it should be VARIABLE (node_kind::VARIABLE = 1)
    // or remain 0 depending on classification rules. The contract is that span fields are NOT zeroed.
    assert_ne!(
        word_at(&typed, 10 + 5),
        0,
        "classifier must not zero span_start for identifier row"
    );
    assert_ne!(
        word_at(&typed, 10 + 6),
        0,
        "classifier must not zero span_len for identifier row"
    );
}

// ---------------------------------------------------------------------------
// 4. Expression-shape boundary contracts
// ---------------------------------------------------------------------------

#[test]
fn expr_shape_empty_vast_produces_empty() {
    let shape = reference_c11_build_expression_shape_nodes(&[], &[]);
    assert!(
        shape.is_empty(),
        "empty vast must produce empty shape buffer"
    );
}

#[test]
fn expr_shape_unknown_kind_gets_none_shape() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = 0xDEAD_BEEF;
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    let shape = reference_c11_build_expression_shape_nodes(&vast_bytes, &typed);
    assert_eq!(shape.len(), 8 * 4, "one shape row = 8 u32s");
    assert_eq!(
        word_at(&shape, 0),
        C_EXPR_SHAPE_NONE,
        "unknown kind must get SHAPE_NONE"
    );
}

#[test]
fn expr_shape_preserves_source_idx_for_none_shapes() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = TOK_SEMICOLON;
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    let shape = reference_c11_build_expression_shape_nodes(&vast_bytes, &typed);
    assert_eq!(shape.len(), 8 * 4, "one shape row = 8 u32s");
    assert_eq!(
        word_at(&shape, 0),
        C_EXPR_SHAPE_NONE,
        "semicolon must get SHAPE_NONE"
    );
    assert_eq!(
        word_at(&shape, 1),
        SENTINEL,
        "source_idx for NONE shape must be SENTINEL"
    );
}

// ---------------------------------------------------------------------------
// 5. Max-size boundary contracts
// ---------------------------------------------------------------------------

#[test]
fn vast_builder_handles_64_token_stream() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_PLUS).take(64).collect();
    let tok_starts: Vec<u32> = (0..64).collect();
    let tok_lens: Vec<u32> = vec![1; 64];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        raw.len(),
        64 * VAST_STRIDE_U32 * 4,
        "64 tokens must produce 64 VAST rows"
    );
}

#[test]
fn classifier_handles_64_node_vast() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_PLUS).take(64).collect();
    let tok_starts: Vec<u32> = (0..64).collect();
    let tok_lens: Vec<u32> = vec![1; 64];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    assert_eq!(
        typed.len(),
        raw.len(),
        "classifier must preserve row count for 64 nodes"
    );
}

#[test]
fn pg_lower_handles_64_node_vast() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_PLUS).take(64).collect();
    let tok_starts: Vec<u32> = (0..64).collect();
    let tok_lens: Vec<u32> = vec![1; 64];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);
    assert_eq!(
        pg.len(),
        64 * PG_STRIDE_U32 * 4,
        "PG lowerer must emit 64 rows for 64 nodes"
    );
}

#[test]
fn expr_shape_handles_64_node_vast() {
    let tok_types: Vec<u32> = std::iter::repeat(TOK_PLUS).take(64).collect();
    let tok_starts: Vec<u32> = (0..64).collect();
    let tok_lens: Vec<u32> = vec![1; 64];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let shape = run_reference_expr_shape(&raw, &typed);
    assert_eq!(
        shape.len(),
        64 * 8 * 4,
        "expr-shape must emit 64 rows for 64 nodes"
    );
}

#[test]
fn token_stream_bounded_by_ast_max_tok_scan_contract() {
    // 256 tokens is well under the limit; the contract is that count <= limit.
    let tok_types: Vec<u32> = std::iter::repeat(TOK_STAR).take(256).collect();
    let tok_starts: Vec<u32> = (0..256).collect();
    let tok_lens: Vec<u32> = vec![1; 256];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let node_count = node_count_from_vast(&raw);
    assert_eq!(node_count, 256);
    assert!(
        node_count <= C11_AST_MAX_TOK_SCAN,
        "node count must respect C11_AST_MAX_TOK_SCAN"
    );
}

// ---------------------------------------------------------------------------
// 6. No silent empty / default outputs
// ---------------------------------------------------------------------------

#[test]
fn pg_lower_does_not_silently_default_kind_for_valid_input() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = node_kind::VARIABLE;
    vast[5] = 100;
    vast[6] = 5;
    let vast_bytes = u32_bytes(&vast);
    let pg = run_reference_pg_lower(&vast_bytes);
    assert_eq!(
        word_at(&pg, 0),
        node_kind::VARIABLE,
        "kind must not be silently defaulted"
    );
    assert_eq!(
        word_at(&pg, 1),
        100,
        "span_start must not be silently zeroed"
    );
}

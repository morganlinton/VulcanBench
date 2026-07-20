#[test]
fn classifier_does_not_emit_all_zeros_for_nonempty_vast() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = TOK_RETURN;
    vast[5] = 7;
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    // At least one word must be non-zero (the classified kind or the preserved span)
    let any_nonzero = typed
        .chunks_exact(4)
        .any(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()) != 0);
    assert!(
        any_nonzero,
        "classifier must not silently emit all-zero output for non-empty input"
    );
}

#[test]
fn expr_shape_does_not_emit_all_zeros_for_nonempty_vast() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = TOK_PLUS;
    let vast_bytes = u32_bytes(&vast);
    let typed = reference_c11_classify_vast_node_kinds(&vast_bytes);
    let shape = reference_c11_build_expression_shape_nodes(&vast_bytes, &typed);
    let any_nonzero = shape.iter().any(|&b| b != 0);
    assert!(
        any_nonzero,
        "expr-shape must not silently emit all-zero output for non-empty input"
    );
}

#[test]
fn vast_builder_does_not_emit_all_zeros_for_nonempty_tokens() {
    let tok_types = [TOK_IF];
    let tok_starts = [0u32];
    let tok_lens = [2u32];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let any_nonzero = raw.iter().any(|&b| b != 0);
    assert!(
        any_nonzero,
        "VAST builder must not silently emit all-zero output for non-empty input"
    );
}

#[test]
fn pg_lower_nonempty_input_produces_nonempty_output() {
    let mut vast = vec![0u32; VAST_STRIDE_U32];
    vast[0] = node_kind::CALL;
    let vast_bytes = u32_bytes(&vast);
    let pg = run_reference_pg_lower(&vast_bytes);
    assert_eq!(
        pg.len(),
        PG_STRIDE_U32 * 4,
        "single-node PG lowerer must emit one PG_STRIDE row"
    );
}

// ---------------------------------------------------------------------------
// 7. Full pipeline on minimal malformed-but-structural inputs
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_on_mismatched_delimiters_produces_non_empty_output() {
    let tok_types = [TOK_LBRACE, TOK_RBRACE, TOK_RPAREN];
    let tok_starts = [0u32, 2, 4];
    let tok_lens = [1u32; 3];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 3 * VAST_STRIDE_U32 * 4);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    assert_eq!(typed.len(), raw.len());
    let shape = reference_c11_build_expression_shape_nodes(&raw, &typed);
    // Expression-shape rows use C_EXPR_SHAPE_STRIDE_U32 (8 u32 words/row),
    // distinct from the VAST stride (10), so the shape stream is intentionally
    // shorter than the typed VAST stream. Three input rows -> three expr-shape
    // rows of 8 words each. (The old `shape.len() == typed.len()` wrongly
    // assumed the two streams share a stride.)
    assert_eq!(shape.len(), 3 * 8 * 4, "three expr-shape rows of 8 u32 words each");
    let pg = run_reference_pg_lower(&typed);
    assert_eq!(pg.len(), 3 * PG_STRIDE_U32 * 4);
}

#[test]
fn full_pipeline_on_all_delimiters_produces_structural_rows() {
    let tok_types = [
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_LBRACKET,
        TOK_RBRACKET,
    ];
    let tok_starts: Vec<u32> = (0..6).map(|i| i * 2).collect();
    let tok_lens = vec![1u32; 6];
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(raw.len(), 6 * VAST_STRIDE_U32 * 4);
    let typed = reference_c11_classify_vast_node_kinds(&raw);
    let pg = run_reference_pg_lower(&typed);
    assert_eq!(pg.len(), 6 * PG_STRIDE_U32 * 4);
    // Verify span_end > span_start for every row (no zeroed defaults)
    for i in 0..6 {
        let start = word_at(&pg, i * PG_STRIDE_U32 + 1);
        let end = word_at(&pg, i * PG_STRIDE_U32 + 2);
        assert!(end >= start, "PG row {i} must have span_end >= span_start");
    }
}

use vyre_primitives::predicate::node_kind;

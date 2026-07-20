use super::*;

#[test]
fn pg_lower_preserves_corpus_kinds_and_spans() {
    for case in CORPUS_CASES {
        let (tok_types, tok_starts, tok_lens) = (case.fixture)();
        let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        let typed = reference_c11_classify_vast_node_kinds(&raw);
        let pg = reference_ast_to_pg_nodes(&typed);

        let node_count = typed.len() / (VAST_STRIDE_U32 * 4);
        assert_eq!(
            pg.len(),
            node_count * PG_STRIDE_U32 * 4,
            "{}: PG buffer size must match node count",
            case.name
        );

        // Every non-zero typed kind must survive lowering with the same kind
        // and the same span start.
        for idx in 0..node_count {
            let vast_kind = word_at(&typed, idx * VAST_STRIDE_U32);
            let pg_kind = pg_word_at(&pg, idx, 0);
            let vast_start = word_at(&typed, idx * VAST_STRIDE_U32 + 5);
            let pg_start = pg_word_at(&pg, idx, 1);

            assert_eq!(
                pg_kind, vast_kind,
                "{}: kind drift at row {idx}: PG={pg_kind} VAST={vast_kind}",
                case.name
            );
            assert_eq!(
                pg_start, vast_start,
                "{}: span start drift at row {idx}: PG={pg_start} VAST={vast_start}",
                case.name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// GPU parity  -  VAST builder
// ---------------------------------------------------------------------------

#[test]
fn gpu_parity_vast_builder_nested_anonymous_aggregates() {
    let (tok_types, tok_starts, tok_lens) = fixture_nested_anonymous_aggregates();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for nested anonymous aggregates"
    );
}

#[test]
fn gpu_parity_vast_builder_function_pointer_array() {
    let (tok_types, tok_starts, tok_lens) = fixture_function_pointer_array();
    let expected = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let gpu = run_gpu_vast_builder(&tok_types, &tok_starts, &tok_lens);
    assert_eq!(
        gpu, expected,
        "GPU VAST builder must match CPU for function pointer array"
    );
}


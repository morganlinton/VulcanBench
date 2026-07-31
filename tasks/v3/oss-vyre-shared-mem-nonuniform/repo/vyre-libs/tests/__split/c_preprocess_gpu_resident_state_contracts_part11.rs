use super::*;

#[test]
fn directive_metadata_rejects_span_not_covering_logical_row() {
    let source = b"#define FOO \\\n1\nx";
    let first_newline = source.iter().position(|b| *b == b'\n').unwrap();
    let err = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[first_newline as u32],
        source,
        &[],
    )
    .expect_err("short span must fail loudly");
    assert_eq!(
        err.message,
        "Fix: TOK_PREPROC span must include the full phase-2 spliced directive row"
    );
}

#[test]
fn directive_metadata_null_directive_has_zero_keyword_len() {
    let source = b"#\n";
    let (kinds, values) = reference_c_preprocessor_directive_metadata(
        &[TOK_PREPROC],
        &[0],
        &[source.len() as u32],
        source,
        &[],
    )
    .expect("null directive must classify");
    assert_eq!(kinds, vec![TOK_PP_NULL]);
    assert_eq!(values, vec![0]);
}

// ---------------------------------------------------------------------------
// 5. Expansion queue contracts
// ---------------------------------------------------------------------------

#[test]
fn expansion_queue_emits_source_ordered_tokens() {
    let mut fixture = DynamicFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_STAR]);
    fixture.insert(TOK_STAR, 514, &[TOK_PLUS]);

    let outputs = run_dynamic(&[TOK_IDENTIFIER, TOK_STAR, TOK_IDENTIFIER], &fixture, 8)
        .expect("ordered expansion must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    // First IDENTIFIER → [INTEGER, STAR]; STAR → [PLUS]; second IDENTIFIER → [INTEGER, STAR]
    assert_eq!(count, vec![5]);
    assert_eq!(
        &out[..5],
        &[TOK_INTEGER, TOK_STAR, TOK_PLUS, TOK_INTEGER, TOK_STAR]
    );
}


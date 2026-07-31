use super::*;

#[test]
fn expansion_queue_accumulates_warp_base_per_token() {
    // Each token's warp_base_idx is the sum of all prior emit_counts.
    // 3-token input where first expands to 2, second passes through (1), third to 2.
    let mut fixture = DynamicFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[TOK_INTEGER, TOK_INTEGER]);

    let outputs = run_dynamic(&[TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER], &fixture, 8)
        .expect("warp-base accumulation must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![5]);
    assert_eq!(
        &out[..5],
        &[TOK_INTEGER, TOK_INTEGER, TOK_PLUS, TOK_INTEGER, TOK_INTEGER]
    );
}

#[test]
fn expansion_queue_zero_length_replacement_removes_token() {
    let mut fixture = DynamicFixture::empty();
    fixture.insert(TOK_IDENTIFIER, 512, &[]);

    let outputs = run_dynamic(&[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON], &fixture, 8)
        .expect("zero-length replacement must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![2]);
    assert_eq!(&out[..2], &[TOK_INT, TOK_SEMICOLON]);
}

#[test]
fn expansion_queue_passthrough_for_unmapped_tokens() {
    let fixture = DynamicFixture::empty();
    let input = &[TOK_INT, TOK_PLUS, TOK_IDENTIFIER];
    let outputs = run_dynamic(input, &fixture, 8).expect("passthrough must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![3]);
    assert_eq!(&out[..3], input);
}

// ---------------------------------------------------------------------------
// 6. Overflow diagnostics contracts
// ---------------------------------------------------------------------------


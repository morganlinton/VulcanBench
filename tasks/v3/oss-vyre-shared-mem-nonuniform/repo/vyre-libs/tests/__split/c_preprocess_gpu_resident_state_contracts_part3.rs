use super::*;

#[test]
fn macro_replacement_range_must_be_inside_table_bounds() {
    let mut fixture = DynamicFixture::empty();
    // Set up a macro whose replacement_offset + size exceeds TABLE_SLOTS.
    // We manually set vals[slot] and sizes[macro_idx] without writing the
    // actual replacement tokens, because the trap fires before the read.
    let slot = hash_token(TOK_IDENTIFIER);
    fixture.keys[slot] = TOK_IDENTIFIER;
    let macro_idx = TABLE_SLOTS - 1;
    fixture.vals[slot] = macro_idx as u32;
    fixture.sizes[macro_idx] = 2; // emit_count crosses the table boundary.

    let result = catch_unwind(AssertUnwindSafe(|| {
        run_dynamic(&[TOK_IDENTIFIER], &fixture, 8)
    }));
    let eval = result.expect("out-of-bounds replacement must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

// ---------------------------------------------------------------------------
// 2. Function-like macro arg arena contracts
// ---------------------------------------------------------------------------

#[test]
fn function_like_macro_arg_arena_bounds_are_zero_initialised() {
    // Before any argument scanning, every arg start/end must equal macro_scan_base.
    // We verify indirectly: a zero-argument call with immediate `)` must succeed.
    let stream = TokenStream {
        source: b"F()",
        types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_RPAREN],
        starts: vec![0, 1, 2],
        lens: vec![1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 0, &[]);

    let outputs = run_named(&stream, &fixture, 8).expect("zero-arg call must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![0]);
    assert_eq!(&out[..0], &[] as &[u32]);
}

#[test]
fn function_like_macro_arg_arena_tracks_multiple_args() {
    let stream = TokenStream {
        source: b"ADD(a,b)",
        types: vec![
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_COMMA,
            TOK_IDENTIFIER,
            TOK_RPAREN,
        ],
        starts: vec![0, 3, 4, 5, 6, 7],
        lens: vec![3, 1, 1, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    // replacement: arg0 + arg1
    fixture.insert(
        b"ADD",
        512,
        C_MACRO_KIND_FUNCTION_LIKE,
        2,
        &[(0, 0), (TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL), (0, 1)],
    );

    let outputs = run_named(&stream, &fixture, 8).expect("two-arg call must succeed");
    let out = decode_u32_words(&outputs[0].to_bytes());
    let count = decode_u32_words(&outputs[1].to_bytes());
    assert_eq!(count, vec![3]);
    assert_eq!(&out[..3], &[TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER]);
}

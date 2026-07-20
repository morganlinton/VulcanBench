use super::*;

#[test]
fn collision_safe_macro_name_long_name_exceeds_pool_bounds_fails_loudly() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let mut fixture = NamedFixture::empty();
    // Manually corrupt the name lens to exceed NAME_POOL_BYTES.
    let hash = fnv1a32(b"FOO");
    let slot = macro_slot(hash);
    fixture.name_hashes[slot] = hash;
    fixture.name_starts[slot] = (NAME_POOL_BYTES - 2) as u32;
    fixture.name_lens[slot] = 10; // exceeds pool
    fixture.vals[slot] = 512;
    fixture.kinds[slot] = C_MACRO_KIND_OBJECT_LIKE;
    fixture.sizes[512] = 1;
    fixture.vals[512] = TOK_INTEGER;

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 4)));
    let eval = result.expect("name-pool overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn collision_safe_macro_name_source_span_out_of_bounds_fails_loudly() {
    let mut fixture = NamedFixture::empty();
    // Corrupt start+len to exceed source length.
    let hash = fnv1a32(b"FOO");
    let slot = macro_slot(hash);
    fixture.name_hashes[slot] = hash;
    fixture.name_starts[slot] = 0;
    fixture.name_lens[slot] = 3;
    // name_words is fine, but we need the source span check to fail.
    // Actually the source span check happens before name comparison.
    // Let's make the source shorter than the token span.
    let short_stream = TokenStream {
        source: b"FO", // 2 bytes, but lens says 3
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    fixture.vals[slot] = 512;
    fixture.kinds[slot] = C_MACRO_KIND_OBJECT_LIKE;
    fixture.sizes[512] = 1;
    fixture.vals[512] = TOK_INTEGER;

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&short_stream, &fixture, 4)));
    let eval = result.expect("source-span out-of-bounds must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

// ---------------------------------------------------------------------------
// 8. Table-driven broad coverage
// ---------------------------------------------------------------------------

#[test]
fn table_directive_metadata_kind_roundtrip() {
    let cases = [
        CPreprocessorDirectiveKind::Null,
        CPreprocessorDirectiveKind::Define,
        CPreprocessorDirectiveKind::Undef,
        CPreprocessorDirectiveKind::Include,
        CPreprocessorDirectiveKind::If,
        CPreprocessorDirectiveKind::Ifdef,
        CPreprocessorDirectiveKind::Ifndef,
        CPreprocessorDirectiveKind::Elif,
        CPreprocessorDirectiveKind::Else,
        CPreprocessorDirectiveKind::Endif,
        CPreprocessorDirectiveKind::Pragma,
        CPreprocessorDirectiveKind::Line,
        CPreprocessorDirectiveKind::Error,
    ];
    for kind in &cases {
        let id = kind.token_id();
        // Every ID must be in the preprocessor directive sub-kind range.
        assert!(
            (203..=215).contains(&id),
            "{:?} id {} out of range",
            kind,
            id
        );
    }
}


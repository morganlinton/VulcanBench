use super::*;

#[test]
fn overflow_function_like_macro_missing_rparen() {
    let stream = TokenStream {
        source: b"F(a,b",
        types: vec![
            TOK_IDENTIFIER,
            TOK_LPAREN,
            TOK_IDENTIFIER,
            TOK_COMMA,
            TOK_IDENTIFIER,
        ],
        starts: vec![0, 1, 2, 3, 4],
        lens: vec![1, 1, 1, 1, 1],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert(b"F", 512, C_MACRO_KIND_FUNCTION_LIKE, 2, &[(0, 0)]);

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("missing-rparen must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn overflow_object_like_macro_replacement_cannot_reference_parameters() {
    // Object-like macros with non-LITERAL replacement_param must trap.
    let stream = TokenStream {
        source: b"OBJ",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let mut fixture = NamedFixture::empty();
    fixture.insert_at_slot_with_hash(
        macro_slot(fnv1a32(b"OBJ")),
        fnv1a32(b"OBJ"),
        b"OBJ",
        512,
        C_MACRO_KIND_OBJECT_LIKE,
        &[(TOK_INTEGER, 0)], // param 0 instead of LITERAL
    );

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("object-like param ref must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

#[test]
fn overflow_named_macro_replacement_range_out_of_bounds() {
    let stream = TokenStream {
        source: b"FOO",
        types: vec![TOK_IDENTIFIER],
        starts: vec![0],
        lens: vec![3],
    };
    let mut fixture = NamedFixture::empty();
    // Manually set up a macro whose replacement_offset + size exceeds TABLE_SLOTS.
    let hash = fnv1a32(b"FOO");
    let slot = macro_slot(hash);
    fixture.name_hashes[slot] = hash;
    fixture.install_name(slot, b"FOO");
    let macro_idx = TABLE_SLOTS - 1;
    fixture.vals[slot] = macro_idx as u32;
    fixture.kinds[slot] = C_MACRO_KIND_OBJECT_LIKE;
    fixture.sizes[macro_idx] = 2; // repl_size crosses the table boundary.

    let result = catch_unwind(AssertUnwindSafe(|| run_named(&stream, &fixture, 8)));
    let eval = result.expect("replacement range overflow must return an error, not panic");
    let err = eval.expect_err("expected reference evaluation failure");
        assert!(
            err.to_string().contains("reference dispatch trapped"),
            "unexpected error: {err}"
        );
}

// ---------------------------------------------------------------------------
// 7. Collision-safe macro name contracts
// ---------------------------------------------------------------------------

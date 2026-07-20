// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_alignas_on_variable_stays_raw_and_classifies() {
    let fix = fixture_alignas_on_variable();
    let typed = classify_fixture(&fix);

    assert_eq!(fix.tok_types[0], TOK_ALIGNAS, "_Alignas must promote");
    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        0,
        "_Alignas must stay raw syntax"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "x must be VARIABLE"
    );
}

#[test]
fn cpu_aligned_attribute_on_array_classifies() {
    let fix = fixture_aligned_attribute_on_array();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "__attribute__ must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![3],
        "aligned must classify"
    );
    assert_eq!(
        word_at(&typed, 10 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "arr must be VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "arr brackets must be ARRAY_DECL"
    );
}

#[test]
fn gpu_parity_aligned_attribute_on_array() {
    let fix = fixture_aligned_attribute_on_array();
    assert_full_pipeline_parity(&fix, "aligned_attribute_on_array");
}

// ---------------------------------------------------------------------------
// 7. Designated initializers
// ---------------------------------------------------------------------------

fn fixture_nested_designated_init_complex() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_STRUCT),
        FixtureToken::new("Outer", TOK_IDENTIFIER),
        FixtureToken::new("o", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("inner", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("arr", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new("...", TOK_ELLIPSIS),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new(".", TOK_DOT),
        FixtureToken::new("flag", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}


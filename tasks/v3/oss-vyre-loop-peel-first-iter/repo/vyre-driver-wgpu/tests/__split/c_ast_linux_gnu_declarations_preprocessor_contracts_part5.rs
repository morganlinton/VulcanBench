// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_typeof_unqual_simple_classifies() {
    let fix = fixture_typeof_unqual_simple();
    let typed = classify_fixture(&fix);

    assert_eq!(
        fix.tok_types[0], TOK_GNU_TYPEOF_UNQUAL,
        "__typeof_unqual__ must promote"
    );
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "x must be VARIABLE"
    );
}

#[test]
fn cpu_typeof_array_declarator_classifies() {
    let fix = fixture_typeof_array_declarator();
    let typed = classify_fixture(&fix);

    assert_eq!(fix.tok_types[0], TOK_GNU_TYPEOF, "typeof must promote");
    assert_eq!(
        word_at(&typed, 7 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "arr must be VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 8 * VAST_STRIDE_U32),
        C_AST_KIND_ARRAY_DECL,
        "arr brackets must be ARRAY_DECL"
    );
}

#[test]
fn gpu_parity_typeof_array_declarator() {
    let fix = fixture_typeof_array_declarator();
    assert_full_pipeline_parity(&fix, "typeof_array_declarator");
}

// ---------------------------------------------------------------------------
// 6. alignas / aligned
// ---------------------------------------------------------------------------

fn fixture_alignas_on_variable() -> Fixture {
    build_fixture(&[
        FixtureToken::new("_Alignas", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_aligned_attribute_on_array() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("16", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("arr", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}


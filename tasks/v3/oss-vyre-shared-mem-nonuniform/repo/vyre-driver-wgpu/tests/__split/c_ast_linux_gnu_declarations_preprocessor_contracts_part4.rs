// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn gpu_parity_attribute_on_function_pointer_typedef() {
    let fix = fixture_attribute_on_function_pointer_typedef();
    assert_full_pipeline_parity(&fix, "attribute_on_function_pointer_typedef");
}

// ---------------------------------------------------------------------------
// 4. __auto_type
// ---------------------------------------------------------------------------

fn fixture_auto_type_pointer_init() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__auto_type", TOK_IDENTIFIER),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("&", TOK_AMP),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn cpu_auto_type_pointer_init_classifies() {
    let fix = fixture_auto_type_pointer_init();
    let typed = classify_fixture(&fix);

    assert_eq!(
        fix.tok_types[0], TOK_GNU_AUTO_TYPE,
        "__auto_type must promote"
    );
    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        0,
        "__auto_type specifier must stay raw syntax"
    );
    assert_eq!(
        word_at(&typed, 1 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "p must be VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_UNARY_EXPR,
        "& must be unary expr"
    );
}

#[test]
fn gpu_parity_auto_type_pointer_init() {
    let fix = fixture_auto_type_pointer_init();
    assert_full_pipeline_parity(&fix, "auto_type_pointer_init");
}

// ---------------------------------------------------------------------------
// 5. typeof / typeof_unqual
// ---------------------------------------------------------------------------

fn fixture_typeof_unqual_simple() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__typeof_unqual__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_typeof_array_declarator() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typeof", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("arr", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}


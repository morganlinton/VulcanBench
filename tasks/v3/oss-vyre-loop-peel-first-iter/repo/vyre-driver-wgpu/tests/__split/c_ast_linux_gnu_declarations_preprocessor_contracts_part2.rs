// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_enum_tag_forward_declaration() {
    let fix = fixture_enum_tag_forward_declaration();
    let typed = classify_fixture(&fix);

    assert_eq!(
        word_at(&typed, 0 * VAST_STRIDE_U32),
        C_AST_KIND_ENUM_DECL,
        "enum keyword must classify"
    );
    assert_eq!(
        word_at(&typed, 1 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "enum tag must classify as VARIABLE"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        C_AST_KIND_ENUM_DECL,
        "second enum keyword must classify"
    );
    assert_eq!(
        word_at(&typed, 5 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "enum variable must classify as VARIABLE"
    );
}

#[test]
fn gpu_parity_struct_tag_forward_declaration() {
    let fix = fixture_struct_tag_forward_declaration();
    assert_full_pipeline_parity(&fix, "struct_tag_forward_declaration");
}

#[test]
fn gpu_parity_enum_tag_forward_declaration() {
    let fix = fixture_enum_tag_forward_declaration();
    assert_full_pipeline_parity(&fix, "enum_tag_forward_declaration");
}

// ---------------------------------------------------------------------------
// 3. GNU __attribute__
// ---------------------------------------------------------------------------

fn fixture_attribute_on_struct_definition() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("packed", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("S", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_on_function_pointer_typedef() -> Fixture {
    build_fixture(&[
        FixtureToken::new("typedef", TOK_IDENTIFIER),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("fp", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}


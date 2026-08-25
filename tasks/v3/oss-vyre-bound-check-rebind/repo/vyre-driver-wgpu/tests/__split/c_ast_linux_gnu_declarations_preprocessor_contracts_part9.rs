// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_flexible_array_member_classifies() {
    let fix = fixture_flexible_array_member();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_STRUCT_DECL),
        vec![0],
        "struct keyword must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FIELD_DECL),
        vec![4, 7],
        "n and arr must be FIELD_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ARRAY_DECL),
        vec![8],
        "empty brackets must be ARRAY_DECL"
    );
}

#[test]
fn gpu_parity_flexible_array_member() {
    let fix = fixture_flexible_array_member();
    assert_full_pipeline_parity(&fix, "flexible_array_member");
}

// ---------------------------------------------------------------------------
// 10. Function pointers
// ---------------------------------------------------------------------------

fn fixture_signal_function() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("signal", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("sig", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("void", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("func", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn cpu_signal_function_classifies() {
    let fix = fixture_signal_function();
    let typed = classify_fixture(&fix);

    assert_eq!(
        row_indices(&typed, C_AST_KIND_POINTER_DECL),
        vec![2, 10],
        "both stars must be POINTER_DECL"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_FUNCTION_DECLARATOR),
        vec![4, 13, 18],
        "function declarators expected"
    );
    assert_eq!(
        word_at(&typed, 3 * VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "signal must be FUNCTION_DECL"
    );
    assert_eq!(
        word_at(&typed, 11 * VAST_STRIDE_U32),
        node_kind::VARIABLE,
        "func must be VARIABLE"
    );
}


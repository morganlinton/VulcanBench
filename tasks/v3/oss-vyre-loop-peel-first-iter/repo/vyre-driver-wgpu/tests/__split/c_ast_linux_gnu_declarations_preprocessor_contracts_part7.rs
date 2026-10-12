// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn cpu_nested_designated_init_complex_classifies() {
    let fix = fixture_nested_designated_init_complex();
    let typed = classify_fixture(&fix);

    let lists = row_indices(&typed, C_AST_KIND_INITIALIZER_LIST);
    assert_eq!(
        lists.len(),
        3,
        "outer, middle, inner initializer lists must classify; got {lists:?}"
    );

    let members = row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert!(
        members.len() >= 3,
        "dot designators must classify; got {members:?}"
    );

    let arrays = row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    assert!(
        arrays.len() >= 2,
        "array designators must classify; got {arrays:?}"
    );

    let ranges = row_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR);
    assert_eq!(ranges, vec![21], "range designator ... must classify");

    let assigns = row_indices(&typed, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        assigns.len() >= 3,
        "assignments in designators must classify; got {assigns:?}"
    );
}

#[test]
fn pg_lower_preserves_nested_designated_init_complex() {
    let fix = fixture_nested_designated_init_complex();
    let typed = classify_fixture(&fix);
    let pg = reference_ast_to_pg_nodes(&typed);

    for idx in row_indices(&typed, C_AST_KIND_INITIALIZER_LIST) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_INITIALIZER_LIST);
    }
    for idx in row_indices(&typed, C_AST_KIND_MEMBER_ACCESS_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_MEMBER_ACCESS_EXPR);
    }
    for idx in row_indices(&typed, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR);
    }
    for idx in row_indices(&typed, C_AST_KIND_RANGE_DESIGNATOR_EXPR) {
        assert_pg_preserves_row(&typed, &pg, &fix, idx, C_AST_KIND_RANGE_DESIGNATOR_EXPR);
    }
}

#[test]
fn gpu_parity_nested_designated_init_complex() {
    let fix = fixture_nested_designated_init_complex();
    assert_full_pipeline_parity(&fix, "nested_designated_init_complex");
}

// ---------------------------------------------------------------------------
// 8. Bitfields
// ---------------------------------------------------------------------------

fn fixture_bitfield_mixed_with_attribute() -> Fixture {
    build_fixture(&[
        FixtureToken::new("struct", TOK_IDENTIFIER),
        FixtureToken::new("Flags", TOK_IDENTIFIER),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("a", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("4", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("packed", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("unsigned", TOK_IDENTIFIER),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("0", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("int", TOK_IDENTIFIER),
        FixtureToken::new("b", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}


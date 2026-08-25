// (use super::* removed  -  parts now share crate-root scope via flat include)

#[test]
fn asm_goto_classifies_template_and_labels() {
    let fix = fixture_asm_goto_with_labels();
    assert_eq!(fix.tok_types[0], TOK_GNU_ASM, "__asm__ must promote");
    assert_full_pipeline_parity(&fix, "asm_goto_with_labels");

    let typed = classify(&fix);
    assert_eq!(row_indices(&typed, C_AST_KIND_INLINE_ASM), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![3]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_GOTO_LABELS),
        vec![8, 10],
        "asm goto labels must classify distinctly"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "goto after asm is a qualifier, not a standalone GOTO_STMT"
    );
}

#[test]
fn asm_extended_io_classifies_operands_and_clobbers() {
    let fix = fixture_asm_extended_io_clobbers();
    assert_full_pipeline_parity(&fix, "asm_extended_io_clobbers");

    let typed = classify(&fix);
    assert_eq!(row_indices(&typed, C_AST_KIND_INLINE_ASM), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ASM_TEMPLATE), vec![3]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_OUTPUT_OPERAND),
        vec![6],
        "output operand must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_INPUT_OPERAND),
        vec![11],
        "input operand must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST),
        vec![15],
        "clobber must classify"
    );
}

// ---------------------------------------------------------------------------
// 2. GNU attributes (cleanup, alias, aligned, section)
// ---------------------------------------------------------------------------

fn fixture_attribute_cleanup() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("cleanup", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("free", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_alias() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("alias", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\"real_sym\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("wrapper", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_attribute_aligned_and_section() -> Fixture {
    build_fixture(&[
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("aligned", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("64", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("__attribute__", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("section", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("\".data\"", TOK_STRING),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("buf", TOK_IDENTIFIER),
        FixtureToken::new("[", TOK_LBRACKET),
        FixtureToken::new("8", TOK_INTEGER),
        FixtureToken::new("]", TOK_RBRACKET),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

#[test]
fn attribute_cleanup_classifies_distinctly() {
    let fix = fixture_attribute_cleanup();
    assert_eq!(fix.tok_types[0], TOK_GNU_ATTRIBUTE);
    assert_full_pipeline_parity(&fix, "attribute_cleanup");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE),
        vec![0],
        "attribute list must classify"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_CLEANUP),
        vec![3],
        "cleanup must classify distinctly"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&11),
        "p must be a VARIABLE"
    );
}

#[test]
fn attribute_alias_classifies_distinctly() {
    let fix = fixture_attribute_alias();
    assert_full_pipeline_parity(&fix, "attribute_alias");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIAS),
        vec![3],
        "alias must classify distinctly"
    );
    assert!(
        row_indices(&typed, node_kind::FUNCTION_DECL).contains(&10),
        "wrapper must be a FUNCTION_DECL"
    );
}

#[test]
fn attribute_aligned_and_section_classify_distinctly() {
    let fix = fixture_attribute_aligned_and_section();
    assert_eq!(fix.tok_types[0], TOK_GNU_ATTRIBUTE);
    assert_eq!(fix.tok_types[9], TOK_GNU_ATTRIBUTE);
    assert_full_pipeline_parity(&fix, "attribute_aligned_and_section");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED),
        vec![3],
        "aligned must classify distinctly"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION),
        vec![12],
        "section must classify distinctly"
    );
    assert!(
        row_indices(&typed, node_kind::VARIABLE).contains(&19),
        "buf must be a VARIABLE"
    );
}

// ---------------------------------------------------------------------------
// 3. Computed goto
// ---------------------------------------------------------------------------

fn fixture_computed_goto_simple() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("*", TOK_STAR),
        FixtureToken::new("p", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("&&", TOK_AND),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("label", TOK_IDENTIFIER),
        FixtureToken::new(":", TOK_COLON),
        FixtureToken::new("return", TOK_RETURN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

#[test]
fn computed_goto_classifies_label_address_expr() {
    let fix = fixture_computed_goto_simple();
    assert_full_pipeline_parity(&fix, "computed_goto_simple");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_GNU_LABEL_ADDRESS_EXPR),
        vec![9],
        "`&&` must classify as GNU_LABEL_ADDRESS_EXPR"
    );
    assert_eq!(
        row_indices(&typed, C_AST_KIND_LABEL_STMT),
        vec![12],
        "label must classify as LABEL_STMT"
    );
    assert!(
        row_indices(&typed, C_AST_KIND_GOTO_STMT).is_empty(),
        "computed goto is not a plain GOTO_STMT"
    );
}

// ---------------------------------------------------------------------------
// 4. __builtin_* constructs
// ---------------------------------------------------------------------------

fn fixture_builtin_expect() -> Fixture {
    build_fixture(&[
        FixtureToken::new("if", TOK_IF),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("__builtin_expect", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_builtin_constant_p() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("__builtin_constant_p", TOK_BUILTIN_CONSTANT_P),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_builtin_choose_expr() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new("__builtin_choose_expr", TOK_BUILTIN_CHOOSE_EXPR),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("1", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("2", TOK_INTEGER),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("3", TOK_INTEGER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_builtin_types_compatible_p() -> Fixture {
    build_fixture(&[
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("z", TOK_IDENTIFIER),
        FixtureToken::new("=", TOK_ASSIGN),
        FixtureToken::new(
            "__builtin_types_compatible_p",
            TOK_BUILTIN_TYPES_COMPATIBLE_P,
        ),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(",", TOK_COMMA),
        FixtureToken::new("long", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
    ])
}

fn fixture_generic_builtin_call() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("__builtin_clz", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

#[test]
fn builtin_expect_classifies_as_distinct_expr() {
    let fix = fixture_builtin_expect();
    assert_full_pipeline_parity(&fix, "builtin_expect");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_EXPECT_EXPR),
        vec![2],
        "__builtin_expect must classify as BUILTIN_EXPECT_EXPR, not CALL"
    );
    assert!(
        row_indices(&typed, node_kind::CALL).is_empty(),
        "must not collapse into generic CALL"
    );
}

#[test]
fn builtin_constant_p_classifies_as_distinct_expr() {
    let fix = fixture_builtin_constant_p();
    assert_eq!(fix.tok_types[3], TOK_BUILTIN_CONSTANT_P);
    assert_full_pipeline_parity(&fix, "builtin_constant_p");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR),
        vec![3],
        "__builtin_constant_p must classify distinctly"
    );
}

#[test]
fn builtin_choose_expr_classifies_as_distinct_expr() {
    let fix = fixture_builtin_choose_expr();
    assert_eq!(fix.tok_types[3], TOK_BUILTIN_CHOOSE_EXPR);
    assert_full_pipeline_parity(&fix, "builtin_choose_expr");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_CHOOSE_EXPR),
        vec![3],
        "__builtin_choose_expr must classify distinctly"
    );
}

#[test]
fn builtin_types_compatible_p_classifies_as_distinct_expr() {
    let fix = fixture_builtin_types_compatible_p();
    assert_eq!(fix.tok_types[3], TOK_BUILTIN_TYPES_COMPATIBLE_P);
    assert_full_pipeline_parity(&fix, "builtin_types_compatible_p");

    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR),
        vec![3],
        "__builtin_types_compatible_p must classify distinctly"
    );
}

#[test]
fn catalog_builtin_call_classifies_as_intrinsic() {
    let fix = fixture_generic_builtin_call();
    assert_full_pipeline_parity(&fix, "generic_builtin_call");

    let expected_kind = try_classify_gnu_builtin_name(b"__builtin_clz")
        .expect("__builtin_clz catalog lookup must not fail")
        .expect("__builtin_clz must be present in the GNU builtin catalog");
    let typed = classify(&fix);
    assert_eq!(
        row_indices(&typed, expected_kind),
        vec![6],
        "cataloged __builtin_clz must classify as its intrinsic kind"
    );
    assert!(
        !row_indices(&typed, node_kind::CALL).contains(&6),
        "cataloged __builtin_clz must not collapse into generic CALL"
    );
}

// ---------------------------------------------------------------------------
// 5. _Atomic
// ---------------------------------------------------------------------------

fn fixture_atomic_qualifier() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Atomic", TOK_ATOMIC),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new("x", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

fn fixture_atomic_type_specifier() -> Fixture {
    build_fixture(&[
        FixtureToken::new("void", TOK_VOID),
        FixtureToken::new("f", TOK_IDENTIFIER),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("{", TOK_LBRACE),
        FixtureToken::new("_Atomic", TOK_ATOMIC),
        FixtureToken::new("(", TOK_LPAREN),
        FixtureToken::new("int", TOK_INT),
        FixtureToken::new(")", TOK_RPAREN),
        FixtureToken::new("y", TOK_IDENTIFIER),
        FixtureToken::new(";", TOK_SEMICOLON),
        FixtureToken::new("}", TOK_RBRACE),
    ])
}

use super::*;

#[test]
fn scope_tree_struct_tag_does_not_shadow_ordinary_variable() {
    let fix = fixture(
        "struct_tag_var",
        &[
            tok(TOK_STRUCT),
            ident("S"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("S"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("S"),
            tok(TOK_ASSIGN),
            ident("S"),
            tok(TOK_PLUS),
            ident("S"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // struct tag S at token 1
    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_NONE },
        "struct tag S must not be a declaration"
    );
    // variable S at token 9
    assert_eq!(
        scope_tree_word_at(&st, 9, 2),
        { DECL_KIND_VARIABLE },
        "variable S must be VARIABLE decl"
    );
    // use of S in f body at token 17
    assert_eq!(
        scope_tree_word_at(&st, 17, 2),
        { DECL_KIND_NONE },
        "use of variable S must be NONE"
    );
}

#[test]
fn scope_tree_enum_tag_coexists_with_ordinary_variable() {
    let fix = fixture(
        "enum_tag_var",
        &[
            tok(TOK_ENUM),
            ident("E"),
            tok(TOK_LBRACE),
            ident("A"),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("E"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("E"),
            tok(TOK_ASSIGN),
            ident("A"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // enum tag E at token 1
    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_NONE },
        "enum tag E must not be a declaration"
    );
    // variable E at token 8
    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_VARIABLE },
        "variable E must be VARIABLE decl"
    );
    // enum constant A at token 3
    assert_eq!(
        scope_tree_word_at(&st, 3, 2),
        { DECL_KIND_ENUM_CONSTANT },
        "enum constant A must be ENUM_CONSTANT"
    );
}

#[test]
fn scope_tree_union_tag_coexists_with_typedef() {
    let fix = fixture(
        "union_tag_typedef",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("U"),
            tok(TOK_SEMICOLON),
            tok(TOK_UNION),
            ident("U"),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_UNION),
            ident("U"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // typedef U at token 2
    assert_eq!(scope_tree_word_at(&st, 2, 2), { DECL_KIND_TYPEDEF });
    // union tag U at token 5
    assert_eq!(
        scope_tree_word_at(&st, 5, 2),
        { DECL_KIND_NONE },
        "union tag U must not be a declaration"
    );
}

// ---------------------------------------------------------------------------
// Enum constants
// ---------------------------------------------------------------------------

#[test]
fn annotation_enum_constant_does_not_interfere_with_typedef_visibility() {
    let fix = fixture(
        "enum_const_td",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("A"),
            tok(TOK_SEMICOLON),
            tok(TOK_ENUM),
            ident("E"),
            tok(TOK_LBRACE),
            ident("A"),
            tok(TOK_COMMA),
            ident("B"),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LPAREN),
            ident("A"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let ann = annotate_cpu(&fix);
    let typed = classify_cpu_annotated(&fix);

    // Enum constant A does not shadow typedef A in current reference model
    // (enum constants are not separately tracked in the typedef annotation pass)
    // The typedef A should remain visible
    assert_ne!(
        flags_at(&ann, 2) & TYPEDEF_FLAG_DECL,
        0,
        "typedef A must be declared"
    );
    // (A) in body should be cast if typedef is visible
    assert_eq!(
        kind_at(&typed, 18),
        C_AST_KIND_CAST_EXPR,
        "(A) must be cast when typedef is visible"
    );
}

#[test]
fn scope_tree_enum_constant_used_as_value_has_no_decl_kind() {
    let fix = fixture(
        "enum_const_use",
        &[
            tok(TOK_ENUM),
            ident("E"),
            tok(TOK_LBRACE),
            ident("A"),
            tok(TOK_COMMA),
            ident("B"),
            tok(TOK_COMMA),
            ident("C"),
            tok(TOK_RBRACE),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RETURN),
            ident("B"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // Enum constants in the enum body are identifiers after TOK_LBRACE
    // A at token 3, B at token 5, C at token 7
    for idx in [3, 5, 7] {
        assert_eq!(
            scope_tree_word_at(&st, idx, 2),
            { DECL_KIND_ENUM_CONSTANT },
            "enum constant at token {idx} must be ENUM_CONSTANT"
        );
    }
    // Use of B in return
    assert_eq!(
        scope_tree_word_at(&st, 16, 2),
        { DECL_KIND_NONE },
        "use of enum constant must be NONE"
    );
}

// ---------------------------------------------------------------------------
// Labels namespace
// ---------------------------------------------------------------------------

#[test]
fn scope_tree_label_declaration_kind_is_label() {
    let fix = fixture(
        "label_decl",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("start"),
            tok(TOK_COLON),
            tok(TOK_GOTO),
            ident("start"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 6, 2),
        { DECL_KIND_LABEL },
        "label 'start' must have LABEL decl kind"
    );
    assert_eq!(
        scope_tree_word_at(&st, 8, 2),
        { DECL_KIND_NONE },
        "goto target use must be NONE"
    );
}

#[test]
fn scope_tree_label_does_not_shadow_ordinary_identifier() {
    let fix = fixture(
        "label_no_shadow",
        &[
            tok(TOK_INT),
            ident("start"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("start"),
            tok(TOK_ASSIGN),
            tok(TOK_INTEGER),
            tok(TOK_SEMICOLON),
            ident("start"),
            tok(TOK_COLON),
            tok(TOK_GOTO),
            ident("start"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    // Variable start at token 1
    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_VARIABLE },
        "global var start must be VARIABLE"
    );
    // Assignment use at token 10
    assert_eq!(
        scope_tree_word_at(&st, 10, 2),
        { DECL_KIND_NONE },
        "assignment lhs must be NONE"
    );
    // Label start at token 12
    assert_eq!(
        scope_tree_word_at(&st, 13, 2),
        { DECL_KIND_LABEL },
        "label start must be LABEL"
    );
    // Goto target at token 14
    assert_eq!(
        scope_tree_word_at(&st, 14, 2),
        { DECL_KIND_NONE },
        "goto target must be NONE"
    );
}

#[test]
fn scope_tree_multiple_labels_same_function() {
    let fix = fixture(
        "multi_labels",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("L1"),
            tok(TOK_COLON),
            ident("L2"),
            tok(TOK_COLON),
            tok(TOK_GOTO),
            ident("L1"),
            tok(TOK_SEMICOLON),
            tok(TOK_GOTO),
            ident("L2"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 6, 2),
        { DECL_KIND_LABEL },
        "L1 must be LABEL"
    );
    assert_eq!(
        scope_tree_word_at(&st, 8, 2),
        { DECL_KIND_LABEL },
        "L2 must be LABEL"
    );
}


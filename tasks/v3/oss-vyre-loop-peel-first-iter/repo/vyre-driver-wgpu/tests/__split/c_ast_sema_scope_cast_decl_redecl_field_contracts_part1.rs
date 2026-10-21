use super::*;

#[test]
fn classifier_typedef_in_cast_is_cast_expr() {
    let fix = fixture(
        "cast_expr",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_LPAREN),
            ident("T"),
            tok(TOK_RPAREN),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    assert_eq!(
        kind_at(&typed, 10),
        C_AST_KIND_CAST_EXPR,
        "(T) where T is typedef must be CAST_EXPR"
    );
}

#[test]
fn classifier_typedef_in_declaration_is_pointer_decl() {
    let fix = fixture(
        "decl_ptr",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    assert_eq!(
        kind_at(&typed, 11),
        C_AST_KIND_POINTER_DECL,
        "T * p in declaration must be POINTER_DECL"
    );
}

#[test]
fn classifier_typedef_in_array_declaration_is_array_decl() {
    let fix = fixture(
        "decl_array",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            ident("a"),
            tok(TOK_LBRACKET),
            tok(TOK_INTEGER),
            tok(TOK_RBRACKET),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    assert_eq!(
        kind_at(&typed, 12),
        C_AST_KIND_ARRAY_DECL,
        "T a[10] bracket must be ARRAY_DECL"
    );
}

#[test]
fn classifier_typedef_in_function_declarator() {
    let fix = fixture(
        "decl_func",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            ident("T"),
            tok(TOK_LPAREN),
            tok(TOK_STAR),
            ident("fp"),
            tok(TOK_RPAREN),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    // int (*fp)(void) style function pointer
    assert_eq!(
        kind_at(&typed, 12),
        C_AST_KIND_POINTER_DECL,
        "function pointer inner star must be POINTER_DECL"
    );
    assert_eq!(
        kind_at(&typed, 15),
        C_AST_KIND_FUNCTION_DECLARATOR,
        "parameter parens must be FUNCTION_DECLARATOR"
    );
}

#[test]
fn classifier_shadowed_typedef_in_declaration_becomes_multiply() {
    let fix = fixture(
        "shadow_decl",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            ident("T"),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    // T * p after T is shadowed must be multiplication, not pointer decl
    assert_eq!(
        kind_at(&typed, 14),
        node_kind::BINARY,
        "shadowed T * p must be BINARY multiply"
    );
}

#[test]
fn classifier_ordinary_variable_in_cast_is_not_cast_expr() {
    let fix = fixture(
        "var_cast",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_LPAREN),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_STAR),
            ident("p"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let typed = classify_cpu_annotated(&fix);
    assert_ne!(
        kind_at(&typed, 9),
        C_AST_KIND_CAST_EXPR,
        "(x) where x is variable must NOT be cast"
    );
    assert_eq!(
        kind_at(&typed, 12),
        node_kind::BINARY,
        "* after (x) must be binary multiply"
    );
}

// ---------------------------------------------------------------------------
// Redeclaration edge cases
// ---------------------------------------------------------------------------

#[test]
fn scope_tree_inner_block_declaration_shadows_outer() {
    let fix = fixture(
        "inner_shadow",
        &[
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_VOID),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    let outer_scope = scope_tree_word_at(&st, 1, 0);
    let inner_scope = scope_tree_word_at(&st, 10, 0);
    assert_ne!(
        outer_scope, inner_scope,
        "inner declaration must have different scope"
    );
    assert_eq!(scope_tree_word_at(&st, 1, 2), { DECL_KIND_VARIABLE });
    assert_eq!(scope_tree_word_at(&st, 10, 2), { DECL_KIND_VARIABLE });
}

#[test]
fn scope_tree_same_scope_redeclaration_both_classified_variable() {
    // C11 forbids redeclaration in the same scope, but the current scope-tree
    // heuristic classifies both as VARIABLE. We assert the actual contract.
    let fix = fixture(
        "same_scope_redecl",
        &[
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_VARIABLE },
        "first x must be VARIABLE"
    );
    assert_eq!(
        scope_tree_word_at(&st, 4, 2),
        { DECL_KIND_VARIABLE },
        "second x must also be VARIABLE (heuristic)"
    );
}

#[test]
fn annotation_typedef_redeclaration_in_same_scope_both_marked_decl() {
    let fix = fixture(
        "td_redecl",
        &[
            tok(TOK_TYPEDEF),
            tok(TOK_INT),
            ident("T"),
            tok(TOK_SEMICOLON),
            tok(TOK_TYPEDEF),
            tok(TOK_CHAR_KW),
            ident("T"),
            tok(TOK_SEMICOLON),
        ],
    );
    let ann = annotate_cpu(&fix);

    assert_ne!(
        flags_at(&ann, 2) & TYPEDEF_FLAG_DECL,
        0,
        "first typedef T must be decl"
    );
    assert_ne!(
        flags_at(&ann, 6) & TYPEDEF_FLAG_DECL,
        0,
        "second typedef T must be decl"
    );
}

#[test]
fn scope_tree_function_prototype_redeclaration() {
    let fix = fixture(
        "func_proto_redecl",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_FUNCTION_DECL },
        "first prototype must be FUNCTION_DECL"
    );
    assert_eq!(
        scope_tree_word_at(&st, 7, 2),
        { DECL_KIND_FUNCTION_DECL },
        "second prototype must be FUNCTION_DECL"
    );
}

#[test]
fn scope_tree_function_definition_after_prototype() {
    let fix = fixture(
        "func_def_after_proto",
        &[
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            tok(TOK_VOID),
            ident("f"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("y"),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let st = scope_tree_for(&fix);

    assert_eq!(
        scope_tree_word_at(&st, 1, 2),
        { DECL_KIND_FUNCTION_DECL },
        "prototype must be FUNCTION_DECL"
    );
    assert_eq!(
        scope_tree_word_at(&st, 8, 2),
        { DECL_KIND_FUNCTION },
        "definition must be FUNCTION"
    );
}

// ---------------------------------------------------------------------------
// Struct field namespaces
// ---------------------------------------------------------------------------

